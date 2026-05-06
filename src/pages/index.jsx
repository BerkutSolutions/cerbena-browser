import React from "react";
import Link from "@docusaurus/Link";
import Layout from "@theme/Layout";
import useBaseUrl from "@docusaurus/useBaseUrl";

export default function Home() {
  const logoUrl = useBaseUrl("/img/logo-512.png");

  return (
    <Layout
      title="Cerbena Browser Documentation"
      description="Cerbena Browser secure browsing platform documentation portal">
      <main className="hero-page">
        <section className="hero-panel hero-panel--home">
          <div>
            <p className="eyebrow">CERBENA BROWSER 1.0.29</p>
            <h1>Cerbena Browser Documentation</h1>
            <p className="hero-copy">
              Unified entry point for isolated profiles, route runtime, DNS filters, traffic gateway,
              sync flows, release preparation, and day-to-day operations.
            </p>
            <div className="hero-actions">
              <Link className="button button--primary button--lg" to="/ru/">
                Open Russian wiki
              </Link>
              <Link className="button button--secondary button--lg" to="/en/">
                Open English docs
              </Link>
              <Link className="button button--secondary button--lg" to="/docs-overview">
                Open docs overview
              </Link>
              <Link
                className="button button--secondary button--lg"
                href="https://github.com/BerkutSolutions/cerbena-browser">
                GitHub
              </Link>
            </div>
          </div>
          <img className="hero-mark" src={logoUrl} alt="Cerbena Browser logo" />
        </section>

        <section className="card-grid">
          <Link className="portal-card" to="/ru/core-docs/profiles/">
            <h2>Profiles and Isolation</h2>
            <p>Profile lifecycle, private runtime, and zero-trust launcher boundary.</p>
          </Link>
          <Link className="portal-card" to="/ru/core-docs/network-routing/">
            <h2>Routing and DNS</h2>
            <p>VPN/Proxy/TOR templates, kill-switch, blocklists, and service filters.</p>
          </Link>
          <Link className="portal-card" to="/ru/architecture-docs/architecture/">
            <h2>Architecture</h2>
            <p>Profile contracts, policy engine, local API, and backend enforcement.</p>
          </Link>
          <Link className="portal-card" to="/ru/release-runbook/">
            <h2>Release Runbook</h2>
            <p>Stability gates, release checks, rollout steps, and recovery guidance.</p>
          </Link>
        </section>
      </main>
    </Layout>
  );
}
