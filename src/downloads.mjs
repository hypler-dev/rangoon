import {asset, button, eyebrow} from './site.mjs';
import {icon} from './icons.mjs';

const sourceZip = 'https://github.com/hypler-dev/rangoon/archive/refs/heads/main.zip';

export const downloadsPage = {
  path: '/downloads/',
  title: 'Downloads and systems',
  description: 'Website source is available now. Rangoon application downloads, installers, packages, containers, and services are still in development.',
  body: `<section class="downloads-hero container">
    <div>
      ${eyebrow('Downloads & systems')}
      <h1>Start with the source.<br><span>Follow the system.</span></h1>
      <p>Rangoon is in active development. The website source is available today; the governed agent platform is not yet distributed as an application binary, installer, package, container, or hosted service.</p>
      <div class="button-row">${button('Download website source', sourceZip, 'primary', 'arrow')}${button('View development source', 'https://github.com/hypler-dev/rangoon/', 'secondary', 'github')}</div>
      <p class="hero-footnote"><span class="status-dot"></span> Website source · Application in development</p>
    </div>
    <div class="downloads-hero-art">

      <img src="${asset('rangoon-mobile')}" width="1536" height="1024" alt="Rangoon connecting a laptop, tablet, and phone" fetchpriority="high" />
      <span class="download-callout">${icon('code')} source first</span>
    </div>
  </section>
  <section class="container platform-section">
    <div class="section-heading reveal"><div>${eyebrow('What is available')}<h2>One real download.<br><span>Clear boundaries.</span></h2></div><p>Use the source archive to inspect or run this public website. It is not an installer for the Rangoon application.</p></div>
    <div class="platform-grid">
      <article class="platform-card available reveal"><div class="platform-card-top"><span class="feature-icon">${icon('code')}</span><span class="availability available">Available</span></div><h3>Rangoon website source</h3><p>The public marketing website source on the <code>main</code> branch. Build it locally with Node.js 20 or later.</p><div class="card-actions">${button('Download .zip', sourceZip, 'dark', 'arrow')}<a class="text-link" href="https://github.com/hypler-dev/rangoon/" target="_blank" rel="noopener noreferrer">Browse source ${icon('diagonal')}</a></div></article>
      <article class="platform-card planned reveal"><div class="platform-card-top"><span class="feature-icon">${icon('box')}</span><span class="availability planned">Planned</span></div><h3>Rangoon application</h3><p>There are no released desktop installers, CLI packages, containers, application binaries, or hosted services yet.</p><a class="text-link" href="/roadmap/">Read the roadmap ${icon('arrow')}</a></article>
      <article class="platform-card candidate reveal"><div class="platform-card-top"><span class="feature-icon">${icon('shield')}</span><span class="availability candidate">Candidate</span></div><h3>System targets</h3><p>Candidate platform profiles guide future work. They do not indicate a supported, certified, or downloadable release.</p><a class="text-link" href="#system-profiles">View candidate profiles ${icon('arrow')}</a></article>
    </div>
  </section>
  <section class="container source-workflow reveal">
    <div>${eyebrow('Website source only')}<h2>Build this site locally.</h2><p>These commands build and check the public website. They do not install, start, or configure a Rangoon application runtime.</p></div>
    <div class="code-block"><div><span>${icon('terminal')} Website source workflow</span><button class="copy-button" aria-label="Copy website source commands">${icon('copy')} Copy</button></div><pre><code>git clone https://github.com/hypler-dev/rangoon.git
cd rangoon
npm run build
npm run check
npm test
npm start</code></pre></div>
  </section>
  <section class="container systems-section" id="system-profiles">
    <div class="section-heading reveal"><div>${eyebrow('Future system profiles')}<h2>Designed for real environments.<br><span>Not selected yet.</span></h2></div><p>These profiles come from LNSAT’s distribution architecture, the reference foundation for Rangoon. No initial support profile has been selected for release.</p></div>
    <div class="support-table" role="region" aria-label="Candidate system profiles" tabindex="0">
      <table>
        <thead><tr><th>Platform</th><th>Architecture</th><th>Candidate operating systems</th><th>Status</th></tr></thead>
        <tbody>
          <tr><td><strong>macOS</strong></td><td>Apple Silicon (ARM64)<br>Intel (x86_64)</td><td>Profile under evaluation</td><td><span class="availability candidate">Candidate</span></td></tr>
          <tr><td><strong>Linux</strong></td><td>x86_64<br>ARM64</td><td>Ubuntu 24.04 · Debian 13 · Rocky Linux 9</td><td><span class="availability candidate">Candidate</span></td></tr>
          <tr><td><strong>Windows</strong></td><td>Profile to be determined</td><td>Future platform lane</td><td><span class="availability later">Later</span></td></tr>
        </tbody>
      </table>
    </div>
    <p class="preview-note">Candidate profiles are planning inputs, not system requirements, compatibility commitments, or support announcements.</p>
  </section>
  <section class="mobile-section">
    <div class="container mobile-layout">
      <div class="mobile-copy reveal">
        ${eyebrow('Mobile direction')}
        <h2>Small devices.<br><span>Meaningful boundaries.</span></h2>
        <p>LNSAT mobile support is a source-only architecture plan: iOS and iPadOS Swift shells alongside Android Kotlin shells. The aim is an accountable companion, not an arbitrary mobile shell or background agent daemon.</p>
        <div class="mobile-points">
          <article><span class="feature-icon">${icon('shield')}</span><div><h3>Mobile Policy SDK</h3><p>Planned embedded-app policy evaluation with explicit scope and consent.</p></div></article>
          <article><span class="feature-icon">${icon('workflow')}</span><div><h3>Optional Mobile Edge Worker</h3><p>A planned opt-in component, never assumed to be active.</p></div></article>
          <article><span class="feature-icon">${icon('box')}</span><div><h3>Control Center inventory</h3><p>Planned inventory of enrolled mobile surfaces and their declared state.</p></div></article>
        </div>
      </div>
      <div class="mobile-art reveal"><img src="${asset('rangoon-mobile')}" width="1536" height="1024" loading="lazy" alt="Rangoon among connected desktop and mobile devices" /><span>Mobile architecture<br>is planned</span></div>
    </div>
  </section>
  <section class="container mobile-guardrails reveal">
    <div>${eyebrow('Mobile guardrails')}<h2>Outbound work needs a lease.</h2><p>Future mobile workloads are designed to use explicit outbound leases and respect battery, thermal state, network availability, operating-system permissions, and user consent.</p></div>
    <div class="guardrail-grid"><article><span>${icon('check')}</span><h3>Opt in</h3><p>Mobile execution should begin only with a declared, user-approved purpose.</p></article><article><span>${icon('shield')}</span><h3>Stay bounded</h3><p>Device and platform constraints should limit what runs and when.</p></article><article><span>${icon('eye')}</span><h3>Remain visible</h3><p>Control Center direction keeps enrolled state and workload context inspectable.</p></article></div>
  </section>
  <section class="container downloads-links reveal">
    <div>${eyebrow('Keep exploring')}<h2>Architecture first.<br><span>Evidence all the way down.</span></h2></div>
    <div class="downloads-link-grid"><a href="/lnsat/">${icon('shield')}<span><strong>Meet LNSAT</strong><small>Execution authority and evidence</small></span>${icon('arrow')}</a><a href="/roadmap/">${icon('rocket')}<span><strong>Read the roadmap</strong><small>What is planned, without promises</small></span>${icon('arrow')}</a><a href="/developers/">${icon('code')}<span><strong>Developer direction</strong><small>Extension points in active development</small></span>${icon('arrow')}</a><a href="/ai/">${icon('spark')}<span><strong>AI & agent access</strong><small>Machine-readable project facts</small></span>${icon('arrow')}</a></div>
  </section>`
};
