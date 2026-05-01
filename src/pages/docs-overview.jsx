import React from "react";
import Link from "@docusaurus/Link";
import Layout from "@theme/Layout";

export default function DocsOverviewPage() {
  return (
    <Layout
      title="Documentation Overview"
      description="Reading paths for Cerbena Browser documentation">
      <main className="overview-page">
        <h1>Documentation Overview</h1>
        <p>
          Use this page to jump into the right branch of the wiki for onboarding, operations,
          network hardening, release work, or architecture review.
        </p>

        <div className="card-grid">
          <Link className="portal-card" to="/ru/navigator/">
            <h2>Russian navigator</h2>
            <p>Task-based reading routes for the Russian documentation branch.</p>
          </Link>
          <Link className="portal-card" to="/en/navigator/">
            <h2>English navigator</h2>
            <p>Task-based reading paths for onboarding, operations, and release work.</p>
          </Link>
          <Link className="portal-card" to="/ru/core-docs/ui/">
            <h2>UI and workflows</h2>
            <p>Entry point into the real desktop launcher sections and operator flows.</p>
          </Link>
          <Link className="portal-card" to="/ru/core-docs/dns-and-filters/">
            <h2>DNS and filters</h2>
            <p>Blocklists, service catalog, allow/deny rules, and exception policy.</p>
          </Link>
          <Link className="portal-card" to="/ru/operators/stability-validation/">
            <h2>Stability validation</h2>
            <p>Smoke and stability checks that support release quality.</p>
          </Link>
          <Link className="portal-card" href="https://github.com/BerkutSolutions/cerbena-browser">
            <h2>GitHub</h2>
            <p>Source code, changelog, and the main collaboration entry point.</p>
          </Link>
        </div>
      </main>
    </Layout>
  );
}
