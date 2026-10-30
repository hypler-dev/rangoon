import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { renderDesktopStartup } from '../preview/desktop-startup-view.mjs';

const view = (state, retainedOriginal = 'unassessed') => ({ status: 'ready', view: { state, actions: state === 'conversion_review' ? ['copy', 'cancel', 'quit'] : state === 'handoff' ? [] : ['quit'], retainedOriginal, startupId: '<private-id>', diagnostic: '<private-diagnostic>' } });

test('view renders fixed headings, actions, focus hook and single polite status', () => {
  const html = renderDesktopStartup(view('conversion_review', 'present_unencrypted'), { theme: 'light' });
  assert.match(html, /Copy to encrypted workspace/);
  assert.match(html, /id="desktop-startup-title" tabindex="-1"/);
  assert.equal((html.match(/aria-live="polite"/g) ?? []).length, 1);
  assert.match(html, /data-desktop-startup-action="copy"/);
  assert.match(html, /data-desktop-startup-action="cancel"/);
  assert.match(html, /data-desktop-startup-theme/);
  assert.match(html, /aria-busy="false"/);
  assert.match(html, /The original copy is kept\. It remains unencrypted\./);
  assert.doesNotMatch(html, /private-id|private-diagnostic/);
});

test('warning text is exact, neutral original is not a warning, and handoff has no card', () => {
  assert.match(renderDesktopStartup(view('verify_required', 'not_verified')), /The original copy could not be verified\./);
  assert.doesNotMatch(renderDesktopStartup(view('fresh', 'not_applicable')), /original copy could not be verified/i);
  assert.equal(renderDesktopStartup({ status: 'handoff', view: { state: 'handoff' } }), '');
});

test('rendered controls are keyboard-native buttons and hostile state never interpolates', () => {
  const html = renderDesktopStartup({ status: '<img src=x>', view: { state: '<private>', actions: ['copy'], retainedOriginal: 'not_verified' } });
  assert.match(html, /Workspace unavailable/);
  assert.match(html, /type="button" data-desktop-startup-theme/);
  assert.doesNotMatch(html, /<private>|<img src=x>|onerror=/);
  assert.match(html, /Encrypted portable backup unavailable\./);
  assert.match(html, /aria-busy="false"/);
});

test('checking and working cards expose busy state without progress copy', () => {
  for (const status of ['checking', 'working']) {
    const html = renderDesktopStartup({ status });
    const live = html.match(/<p class="desktop-startup__copy"[^>]*>([^<]*)<\/p>/)?.[1] ?? '';
    assert.match(html, /aria-busy="true"/);
    assert.doesNotMatch(live, /%|ETA|spinner/i);
  }
});

test('startup controls and details summary keep 44px keyboard targets', () => {
  const css = readFileSync(new URL('../preview/desktop-startup.css', import.meta.url), 'utf8');
  assert.match(css, /\.desktop-startup__action\{min-height:44px/);
  assert.match(css, /\.desktop-startup__details summary\{display:flex;align-items:center;min-height:44px/);
  assert.match(css, /\.desktop-startup__details summary:focus-visible\{outline:2px solid var\(--cyan\)/);
});

test('copying reports only its finite native phase, with no progress invention', () => {
  const html = renderDesktopStartup({ status: 'ready', view: { state: 'copying', actions: ['quit'], copyPhase: 'checking_copy', retainedOriginal: 'unassessed' } });
  const live = html.match(/<p class="desktop-startup__copy"[^>]*>([^<]*)<\/p>/)?.[1] ?? '';
  assert.equal(live, 'Rangoon is checking copy.');
  assert.doesNotMatch(live, /%|ETA|spinner|complete/i);
});

test('unavailable native card offers a separate source-free quit action only when enabled', () => {
  const unavailable = renderDesktopStartup({ status: 'unavailable' }, { unavailableQuit: true });
  assert.match(unavailable, /data-desktop-startup-unavailable-quit>Quit Rangoon<\/button>/);
  assert.doesNotMatch(unavailable, /data-desktop-startup-action="quit"/);
  assert.match(unavailable, /This startup route cannot continue safely\./);
  assert.match(renderDesktopStartup({ status: 'unavailable' }, { unavailableQuit: true, quitPending: true }), /data-desktop-startup-unavailable-quit disabled>Quitting Rangoon…<\/button>/);
  assert.doesNotMatch(renderDesktopStartup({ status: 'unavailable' }), /data-desktop-startup-unavailable-quit/);
});
