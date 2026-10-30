const escape = value => String(value ?? '').replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;').replaceAll("'", '&#39;');

export const FOUNDATION_MARKS = Object.freeze({
  vusl: Object.freeze({
    id: 'vusl',
    name: 'VUSL',
    role: 'Shared language',
    state: 'Integration planned',
    detail: 'Shared language for source identity, meaning and translation limits. Maintained independently; Rangoon integration is planned.',
    logoPath: 'assets/foundations/vusl-logo.png',
  }),
  lnsat: Object.freeze({
    id: 'lnsat',
    name: 'LNSAT',
    role: 'Authority engine',
    state: 'Adapter unavailable',
    detail: 'Independent optional authorization and evidence engine. Live connection unavailable; local work remains available.',
    logoPath: 'assets/authority/lnsat-mark.webp',
  }),
});

const foundationCard = foundation => `<article class="foundation-card" data-foundation="${escape(foundation.id)}"><header><span class="foundation-card__mark foundation-card__mark--${escape(foundation.id)}"><img src="${escape(foundation.logoPath)}" alt="" aria-hidden="true"></span><div><h3>${escape(foundation.name)}</h3><p class="foundation-card__role">${escape(foundation.role)}</p></div></header><p class="foundation-card__state">${escape(foundation.state)}</p><details><summary>${escape(foundation.name)} details</summary><p>${escape(foundation.detail)}</p></details></article>`;

export function renderFoundationContext() {
  return `<section class="foundation-context" aria-labelledby="foundation-context-title"><header class="foundation-context__head"><h2 id="foundation-context-title">Foundations</h2></header><div class="foundation-context__cards">${Object.values(FOUNDATION_MARKS).map(foundationCard).join('')}</div></section>`;
}

export function renderFoundationAuthorityLink(href) {
  const foundation = FOUNDATION_MARKS.lnsat;
  return `<a class="authority-context__foundation" href="${escape(href)}"><img src="${escape(foundation.logoPath)}" alt="" aria-hidden="true"><span>${escape(foundation.name)}</span></a>`;
}
