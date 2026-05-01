export const CATEGORY_LABEL_KEYS = {
  artificial_intelligence: "dns.category.artificial_intelligence",
  content_delivery_networks: "dns.category.content_delivery_networks",
  dating_services: "dns.category.dating_services",
  gambling_and_betting: "dns.category.gambling_and_betting",
  games_and_gaming_platforms: "dns.category.games_and_gaming_platforms",
  web_hosting_and_file_sharing: "dns.category.web_hosting_and_file_sharing",
  messaging_services: "dns.category.messaging_services",
  privacy_tools: "dns.category.privacy_tools",
  shopping: "dns.category.shopping",
  social_networks_and_communities: "dns.category.social_networks_and_communities",
  software_development_platforms: "dns.category.software_development_platforms",
  media_and_streaming: "dns.category.media_and_streaming"
};

export const SERVICE_LABELS = {
  "4chan": "4chan",
  "500px": "500px",
  "9gag": "9GAG",
  aliexpress: "AliExpress",
  amazon: "Amazon",
  amino: "Amino",
  betano: "Betano",
  betfair: "Betfair",
  betway: "Betway",
  bilibili: "Bilibili",
  blaze: "Blaze",
  bluesky: "Bluesky",
  box: "Box",
  canais_globo: "Canais Globo",
  chatgpt: "ChatGPT",
  claro: "Claro",
  claude: "Claude",
  clubhouse: "Clubhouse",
  cloudflare: "Cloudflare",
  coolapk: "CoolApk",
  copilot: "Copilot",
  crunchyroll: "Crunchyroll",
  dailymotion: "Dailymotion",
  dating_services: "Dating Services",
  deepseek: "DeepSeek",
  deezer: "Deezer",
  directv_go: "DirecTV Go",
  discord: "Discord",
  discovery_plus: "Discovery+",
  disney_plus: "Disney+",
  douban: "Douban",
  dropbox: "Dropbox",
  ebay: "eBay",
  espn: "ESPN",
  facebook: "Facebook",
  fifa: "FIFA",
  flickr: "Flickr",
  gemini: "Gemini",
  globoplay: "Globoplay",
  google_play_store: "Google Play Store",
  grok: "Grok",
  hbo_max: "HBO Max",
  hulu: "Hulu",
  iheartradio: "iHeartRadio",
  icloud_private_relay: "iCloud Private Relay",
  imgur: "Imgur",
  instagram: "Instagram",
  iqiyi: "iQIYI",
  kakaotalk: "KakaoTalk",
  kik: "Kik",
  kook: "KOOK",
  lazada: "Lazada",
  line: "LINE",
  linkedin: "LinkedIn",
  lionsgate_plus: "Lionsgate+",
  looke: "Looke",
  mail_ru: "Mail.ru",
  manus: "Manus",
  mastodon: "Mastodon",
  max: "MAX",
  max_streaming: "Max",
  mercado_libre: "Mercado Libre",
  meta_ai: "Meta AI",
  microsoft_teams: "Microsoft Teams",
  nebula: "Nebula",
  netflix: "Netflix",
  nvidia: "Nvidia",
  odysee: "Odysee",
  olvid: "Olvid",
  ok_ru: "OK.ru",
  onlyfans: "OnlyFans",
  paramount_plus: "Paramount Plus",
  peacock_tv: "Peacock TV",
  perplexity: "Perplexity",
  pinterest: "Pinterest",
  plenty_of_fish: "Plenty of Fish",
  plex: "Plex",
  pluto_tv: "Pluto TV",
  privacy: "Privacy",
  proton: "Proton",
  rakuten_viki: "Rakuten Viki",
  reddit: "Reddit",
  riot_games: "Riot Games",
  roblox: "Roblox",
  rockstar_games: "Rockstar Games",
  samsung_tv_plus: "Samsung TV Plus",
  shein: "Shein",
  shopee: "Shopee",
  signal: "Signal",
  skype: "Skype",
  slack: "Slack",
  snapchat: "Snapchat",
  soundcloud: "SoundCloud",
  spotify: "Spotify",
  spotify_video: "Spotify Video",
  steam: "Steam",
  telegram_web: "Telegram (Web)",
  temu: "Temu",
  tidal: "Tidal",
  tinder: "Tinder",
  tiktok: "TikTok",
  tumblr: "Tumblr",
  twitch: "Twitch",
  ubisoft: "Ubisoft",
  valorant: "Valorant",
  viber: "Viber",
  vimeo: "Vimeo",
  vivo_play: "Vivo Play",
  vk: "VK",
  vk_com: "VK.com",
  voot: "Voot",
  wargaming: "Wargaming",
  warner_bros_games: "Warner Bros Games",
  wechat: "WeChat",
  weibo: "Weibo",
  whatsapp: "WhatsApp",
  wizz: "Wizz",
  x_twitter: "X (formerly Twitter)",
  xbox_live: "Xbox Live",
  xiaohongshu: "Xiaohongshu",
  yandex: "Yandex",
  youtube: "YouTube",
  zhihu: "Zhihu"
};

export const BLOCKLIST_PRESETS = [
  {
    id: "adguard_dns",
    label: "AdGuard DNS Filter",
    domains: ["doubleclick.net", "googlesyndication.com", "adservice.google.com", "ads.yahoo.com"]
  },
  {
    id: "adguard_mobile",
    label: "AdGuard Mobile Ads",
    domains: ["admob.com", "adservice.google.ru", "ads.mopub.com", "unityads.unity3d.com"]
  },
  {
    id: "adguard_tracking",
    label: "AdGuard Tracking Protection",
    domains: ["google-analytics.com", "hotjar.com", "segment.io", "mixpanel.com"]
  },
  {
    id: "adguard_social",
    label: "AdGuard Social Media",
    domains: ["connect.facebook.net", "platform.twitter.com", "vk.com", "staticxx.facebook.com"]
  },
  {
    id: "adguard_annoyances",
    label: "AdGuard Annoyances",
    domains: ["onesignal.com", "pushwoosh.com", "webpushr.com", "pushengage.com"]
  },
  {
    id: "adguard_malware",
    label: "AdGuard Malware Protection",
    domains: ["malware.test", "phishing.test", "badware.example", "trojan.example"]
  }
];

export function serviceLabel(serviceKey) {
  return SERVICE_LABELS[serviceKey] ?? humanizeKey(serviceKey);
}

export function humanizeKey(value) {
  return String(value)
    .split("_")
    .map((chunk) => {
      if (!chunk) return chunk;
      if (/^\d/.test(chunk)) return chunk.toUpperCase();
      return chunk.charAt(0).toUpperCase() + chunk.slice(1);
    })
    .join(" ");
}
