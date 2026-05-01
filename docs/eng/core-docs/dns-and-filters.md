---
title: DNS, Blocklists, and Service Filters
sidebar_position: 4
---

Cerbena Browser combines profile-scoped DNS, blocklists, a service catalog, and domain rules.

## DNS modes

- `System`
- `Manual`

For `Manual`, standard resolver addresses are supported together with optional `DoH` and `DoT` constraints when the profile defines them.

## What lives on the DNS page

- global DNS blocklists;
- the service catalog presented as an accordion;
- suffix blacklist management;
- profile-scoped allow/deny policies and selected services.

`DNS blocklists` and `Service catalog` are rendered as accordions in the current UI to keep the page lighter to scan.

## Sources of blocking

- profile `domain_denylist`;
- profile `selected_blocklists`;
- profile `selected_services`;
- global DNS blocklists;
- global suffix blacklist;
- manual user blocks from `Traffic`.

## Service catalog categories

- Artificial Intelligence: `ChatGPT`, `Claude`, `Copilot`, `DeepSeek`, `Gemini`, `Grok`, `Perplexity`.
- CDN: `Cloudflare`.
- Dating Services: `Tinder`, `Wizz`, `Plenty of Fish`.
- Gambling and Betting: `Betano`, `Betfair`, `Betway`, `Blaze`.
- Games and Gaming Platforms: `Steam`, `Roblox`, `Riot Games`, `Xbox Live`.
- Web Hosting and File Sharing: `Dropbox`, `Box`, `Imgur`, `Flickr`.
- Messaging Services: `Telegram Web`, `Slack`, `Signal`, `WhatsApp`, `WeChat`.
- Privacy Tools: `iCloud Private Relay`, `Proton`, `Privacy`.
- Shopping: `Amazon`, `AliExpress`, `eBay`, `Temu`, `Shein`.
- Social Networks and Communities: `Discord`, `Reddit`, `TikTok`, `X`, `VK.com`, `Instagram`, `Facebook`.
- Software Development Platforms: `Nvidia`, `Google Play Store`.
- Media and Streaming: `YouTube`, `Netflix`, `Spotify`, `Twitch`, `Disney+`, `HBO Max`.

## Conflict resolution rules

1. Hard constraints win over everything else.
2. An explicit domain deny is stronger than a service allow.
3. An explicit domain allow is stronger than a category block when no hard constraint applies.
4. An exception overrides the base rule inside the same policy layer.
5. If a conflict remains unresolved, deny-by-default applies.

## Operating guidance

- define route policy first;
- then configure DNS servers and blocklists;
- enable service categories and targeted exceptions after that;
- use `Traffic` and backend reason codes to understand disputed outcomes.
