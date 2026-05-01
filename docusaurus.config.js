// @ts-check
const desktopPackage = require("./ui/desktop/package.json");

const REPO_URL = "https://github.com/BerkutSolutions/cerbena-browser";
const isGitHubPagesBuild =
  process.env.GITHUB_ACTIONS === "true" || process.env.DOCS_ENV === "github-pages";
const DOCS_SITE_URL =
  process.env.DOCS_SITE_URL ||
  (isGitHubPagesBuild ? "https://berkutsolutions.github.io" : "http://127.0.0.1:3000");
const DOCS_BASE_URL =
  process.env.DOCS_BASE_URL || (isGitHubPagesBuild ? "/cerbena-browser/" : "/");
const APP_VERSION = desktopPackage.version;

const config = {
  title: "Cerbena Browser Docs",
  tagline: "Standalone secure browsing platform documentation",
  favicon: "img/favicon.ico",
  url: DOCS_SITE_URL,
  baseUrl: DOCS_BASE_URL,
  organizationName: "BerkutSolutions",
  projectName: "cerbena-browser",
  deploymentBranch: "gh-pages",
  onBrokenLinks: "throw",
  trailingSlash: true,
  markdown: {
    hooks: {
      onBrokenMarkdownLinks: "warn",
    },
  },
  presets: [
    [
      "classic",
      {
        docs: false,
        blog: false,
        pages: {},
        theme: {
          customCss: require.resolve("./src/css/custom.css"),
        },
      },
    ],
  ],
  plugins: [
    [
      "@easyops-cn/docusaurus-search-local",
      {
        indexDocs: true,
        indexPages: true,
        docsRouteBasePath: ["ru", "en"],
        docsDir: ["docs/ru", "docs/eng"],
        docsPluginIdForPreferredVersion: "ru",
        language: ["en", "ru"],
        explicitSearchResultPath: true,
        searchBarShortcut: true,
        searchBarPosition: "right",
        hashed: "filename",
        indexBlog: false,
      },
    ],
    [
      "@docusaurus/plugin-content-docs",
      {
        id: "ru",
        path: "docs/ru",
        routeBasePath: "ru",
        exclude: ["README.md"],
        sidebarPath: require.resolve("./sidebars.ru.js"),
        editUrl: `${REPO_URL}/edit/main/docs/ru/`,
      },
    ],
    [
      "@docusaurus/plugin-content-docs",
      {
        id: "en",
        path: "docs/eng",
        routeBasePath: "en",
        exclude: ["README.md"],
        sidebarPath: require.resolve("./sidebars.en.js"),
        editUrl: `${REPO_URL}/edit/main/docs/eng/`,
      },
    ],
  ],
  themeConfig: {
    colorMode: {
      defaultMode: "dark",
      disableSwitch: false,
      respectPrefersColorScheme: true,
    },
    image: "img/logo.png",
    navbar: {
      title: "CERBENA BROWSER",
      logo: {
        alt: "Cerbena Browser logo",
        src: "img/logo-64.png",
      },
      items: [
        { to: "/", label: "Home", position: "left" },
        { to: "/docs-overview", label: "Navigator", position: "left" },
        { to: "/ru/", label: "Russian Wiki", position: "left" },
        { to: "/en/", label: "English Wiki", position: "left" },
        {
          label: "Language",
          position: "right",
          items: [
            { label: "Russian", to: "/ru/" },
            { label: "English", to: "/en/" },
          ],
        },
        {
          label: `Version ${APP_VERSION}`,
          position: "right",
          items: [
            { label: `Current Release ${APP_VERSION}`, to: "/ru/release-runbook/" },
            { label: "CHANGELOG", href: `${REPO_URL}/blob/main/CHANGELOG.md` },
          ],
        },
        { type: "search", position: "right" },
        {
          href: REPO_URL,
          label: "GitHub",
          position: "right",
        },
      ],
    },
    footer: {
      style: "dark",
      links: [
        {
          title: "Documentation",
          items: [
            { label: "Russian Wiki", to: "/ru/" },
            { label: "English Wiki", to: "/en/" },
            { label: "Navigator", to: "/docs-overview" },
          ],
        },
        {
          title: "Platform",
          items: [
            { label: "Profile Isolation", to: "/en/architecture-docs/profile-isolation/" },
            { label: "DNS and Filters", to: "/en/core-docs/dns-and-filters/" },
            { label: "Release Runbook", to: "/en/release-runbook/" },
          ],
        },
        {
          title: "Source",
          items: [{ label: "GitHub", href: REPO_URL }],
        },
      ],
      copyright: `Copyright ${new Date().getFullYear()} Berkut Solutions`,
    },
    prism: {
      additionalLanguages: ["bash", "powershell", "json", "yaml", "rust"],
    },
    docs: {
      sidebar: {
        hideable: true,
        autoCollapseCategories: false,
      },
    },
  },
};

module.exports = config;
