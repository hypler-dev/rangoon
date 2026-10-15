import test from 'node:test';
import assert from 'node:assert/strict';
import { AUTHORITY_PLANNER_ROLES, AUTHORITY_RELATIONSHIPS, createAuthorityPlanner, getAuthorityRole, getAuthorityAlternatives, getAuthorityRelationship, projectAuthorityPlanner } from '../preview/authority-planner-model.mjs';
const ids = ['agentgateway','cerbos','permit','permit-fastmcp','permit-mcp-gateway','invariant-local','invariant-remote','lnsat'];
const assignment = planner => planner.getState().assignments;
test('five distinct optional roles partition the catalog without runtime ordering', () => {
  assert.deepEqual(AUTHORITY_PLANNER_ROLES.map(role => role.id), ['gateway','access-decision','server-middleware','behavior-guardrail','action-authority']);
  assert.deepEqual(AUTHORITY_PLANNER_ROLES.flatMap(role => role.targets).sort(), [...ids].sort());
  assert.equal(getAuthorityRole('cerbos').id, 'access-decision');
  assert.deepEqual(getAuthorityAlternatives('cerbos'), ['permit']);
  assert.deepEqual(getAuthorityAlternatives('lnsat'), []);
  assert.equal(getAuthorityRole('__proto__'), null);
  assert.throws(() => AUTHORITY_PLANNER_ROLES[0].targets.push('hostile'), TypeError);
});
test('all 28 pairs are symmetric and unqualified with literal classification counts', () => {
  assert.equal(AUTHORITY_RELATIONSHIPS.length, 28);
  const counts = {};
  for (let i = 0; i < ids.length; i++) for (const right of ids.slice(i + 1)) {
    const relation = getAuthorityRelationship(ids[i], right);
    assert.equal(relation, getAuthorityRelationship(right, ids[i]));
    assert.deepEqual(Object.keys(relation).sort(), ['ids','kind','label','basis','rationale','requirements','references','compatibility','dropInReplacement','authority','networkAttempted'].sort());
    assert.equal(relation.compatibility, 'unqualified');
    assert.equal(relation.dropInReplacement, false);
    assert.equal(relation.authority, 'none');
    assert.equal(relation.networkAttempted, false);
    assert.ok(relation.references.every(ref => ref.url.startsWith('https://')));
    counts[relation.kind] = (counts[relation.kind] ?? 0) + 1;
    assert.throws(() => { relation.ids[0] = 'hostile'; }, TypeError);
  }
  assert.deepEqual(counts, { complement:19, overlap:2, alternative:3, not_assessed:2, documented_dependency:2 });
  for (const [left,right] of [['permit','permit-fastmcp'],['permit','permit-mcp-gateway']]) assert.equal(getAuthorityRelationship(left,right).basis, 'provider_documentation');
  for (const [left,right] of [['cerbos','permit-fastmcp'],['cerbos','permit-mcp-gateway']]) assert.equal(getAuthorityRelationship(left,right).kind, 'not_assessed');
  assert.equal(getAuthorityRelationship('agentgateway','permit-fastmcp').kind, 'overlap');
  assert.equal(getAuthorityRelationship('invariant-local','invariant-remote').kind, 'alternative');
  assert.equal(getAuthorityRelationship('lnsat','lnsat'), null);
  assert.equal(getAuthorityRelationship('lnsat','<script>'), null);
});
test('projection derives trusted relationships and strips forged authority and assignments', () => {
  const state = projectAuthorityPlanner({ assignments:{gateway:'cerbos','access-decision':'permit',extra:'lnsat'}, pair:['permit','permit-fastmcp'], relationship:{authority:'granted'}, relationships:['forged'], authority:'granted', networkAttempted:true, execution:'running', persistence:'disk', revision:Infinity });
  assert.deepEqual(Object.keys(state).sort(), ['revision','assignments','pair','relationship','relationships','message','authority','execution','networkAttempted','persistence'].sort());
  assert.equal(state.assignments.gateway, null);
  assert.equal(Object.keys(state.assignments).length, 5);
  assert.equal(state.relationship, getAuthorityRelationship('permit','permit-fastmcp'));
  assert.deepEqual(state.relationships, []);
  assert.equal(state.authority, 'none'); assert.equal(state.execution, 'unavailable'); assert.equal(state.networkAttempted, false); assert.equal(state.persistence, 'memory_only'); assert.equal(state.revision, 0);
  assert.deepEqual(projectAuthorityPlanner({pair:['lnsat','lnsat']}).pair, ['lnsat',null]);
  assert.deepEqual(projectAuthorityPlanner(null).pair, [null,null]);
  assert.throws(() => { state.assignments.gateway='agentgateway'; }, TypeError);
});
test('local choice, removal, pair inspection and clear require no provider', () => {
  const planner = createAuthorityPlanner(); let notices = 0;
  const unsubscribe = planner.subscribe(() => { notices++; });
  assert.equal(planner.choose('cerbos','gateway'), false);
  assert.equal(planner.choose('<img>'), false);
  assert.equal(planner.getState().revision, 0);
  assert.equal(planner.choose('agentgateway'), true);
  assert.equal(planner.choose('agentgateway'), false);
  assert.equal(planner.choose('lnsat'), true);
  assert.equal(planner.getState().relationships.length, 1);
  assert.equal(planner.compare('cerbos','permit'), true);
  assert.equal(planner.getState().revision, 2);
  assert.equal(planner.setPair('left','permit'), false);
  assert.equal(planner.setPair('wrong','lnsat'), false);
  assert.equal(planner.setPair('right','lnsat'), true);
  assert.equal(planner.remove('gateway'), true);
  assert.equal(planner.remove('gateway'), false);
  planner.clear();
  assert.ok(Object.values(assignment(planner)).every(value => value === null));
  assert.deepEqual(planner.getState().pair,[null,null]);
  assert.equal(planner.getState().revision, 4);
  unsubscribe(); const count = notices; planner.choose('cerbos'); assert.equal(notices,count);
});
test('occupied slots require bound one-shot replacement, never direct choice', () => {
  const planner = createAuthorityPlanner(); planner.choose('cerbos');
  assert.equal(planner.choose('permit'), false);
  assert.equal(assignment(planner)['access-decision'],'cerbos');
  const token = planner.requestReplacement('permit');
  assert.deepEqual(token,{role:'access-decision',id:'permit',occupant:'cerbos',revision:1});
  assert.throws(() => {token.id='lnsat';},TypeError);
  assert.equal(planner.confirmReplacement({...token}),false);
  assert.equal(planner.confirmReplacement(token),true);
  assert.equal(planner.confirmReplacement(token),false);
  assert.equal(assignment(planner)['access-decision'],'permit');
  assert.equal(planner.getState().revision,2);
  assert.equal(planner.requestReplacement('cerbos','gateway'),null);
  assert.equal(planner.requestReplacement('permit'),null);
});
test('cancel, stale revision, changed occupant and foreign-controller replacement refuse atomically', () => {
  const planner = createAuthorityPlanner(); planner.choose('invariant-local');
  const canceled = planner.requestReplacement('invariant-remote');
  assert.equal(planner.cancelReplacement(canceled),true);
  assert.equal(planner.confirmReplacement(canceled),false);
  const stale = planner.requestReplacement('invariant-remote'); planner.choose('lnsat');
  assert.equal(planner.confirmReplacement(stale),false);
  const changed = planner.requestReplacement('invariant-remote'); planner.remove('behavior-guardrail'); planner.choose('invariant-local');
  assert.equal(planner.confirmReplacement(changed),false);
  const foreign = planner.requestReplacement('invariant-remote');
  const other = createAuthorityPlanner(); other.choose('invariant-local');
  assert.equal(other.confirmReplacement(foreign),false);
  assert.equal(planner.confirmReplacement(foreign),true);
  assert.equal(assignment(other)['behavior-guardrail'],'invariant-local');
  assert.equal(planner.confirmReplacement(null),false);
  assert.equal(planner.cancelReplacement('hostile'),false);
});

test('bounded hostile identifier corpus never reflects payload or changes the local plan', () => {
  const planner = createAuthorityPlanner(); planner.choose('lnsat');
  const initial = planner.getState().assignments;
  const inputs = [null, undefined, 42, {}, '__proto__', 'constructor', '%6cnsat', 'lnsat/extra', '<script>alert(1)</script>', 'x'.repeat(1024), ...Array.from({length:100}, (_, index) => `unknown-${index}`)];
  for (const input of inputs) {
    assert.equal(planner.choose(input), false);
    assert.equal(planner.requestReplacement(input), null);
    assert.equal(planner.compare(input,'cerbos'), false);
    assert.equal(getAuthorityRelationship(input,'cerbos'), null);
    assert.deepEqual(planner.getState().assignments,initial);
    assert.equal(planner.getState().revision,1);
    assert.equal(planner.getState().message,'Plan unchanged. Choose a known target for its matching role.');
  }
});
