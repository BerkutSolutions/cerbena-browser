import { windowsTemplate, macTemplate, linuxTemplate, iosTemplate, androidTemplate } from "./identity-template-catalog-core.js";

const WINDOWS_TEMPLATES = [
  windowsTemplate("7", "edge", 109, "Win 7 - Edge 109"),
  windowsTemplate("7", "firefox", 128, "Win 7 - Firefox 128 ESR"),
  windowsTemplate("7", "chrome", 109, "Win 7 - Chrome 109"),
  windowsTemplate("7", "ie", 11, "Win 7 - Internet Explorer 11"),
  windowsTemplate("8", "edge", 109, "Win 8 - Edge 109"),
  windowsTemplate("8", "firefox", 128, "Win 8 - Firefox 128 ESR"),
  windowsTemplate("8", "chrome", 109, "Win 8 - Chrome 109"),
  windowsTemplate("8", "ie", 11, "Win 8 - Internet Explorer 11"),
  windowsTemplate("8.1", "edge", 109, "Win 8.1 - Edge 109"),
  windowsTemplate("8.1", "firefox", 128, "Win 8.1 - Firefox 128 ESR"),
  windowsTemplate("8.1", "chrome", 109, "Win 8.1 - Chrome 109"),
  windowsTemplate("8.1", "ie", 11, "Win 8.1 - Internet Explorer 11"),
  windowsTemplate("10", "edge", 147, "Win 10 - Edge 147"),
  windowsTemplate("10", "firefox", 140, "Win 10 - Firefox 140 ESR"),
  windowsTemplate("10", "firefox", 128, "Win 10 - Firefox 128 ESR"),
  windowsTemplate("10", "firefox", 150, "Win 10 - Firefox 150"),
  windowsTemplate("10", "chrome", 147, "Win 10 - Chrome 147"),
  windowsTemplate("10", "ie", 11, "Win 10 - Internet Explorer 11")
];

const MAC_TEMPLATES = [
  macTemplate("14", "edge", 147, "macOS 14 - Edge 147"),
  macTemplate("14", "firefox", 140, "macOS 14 - Firefox 140 ESR"),
  macTemplate("14", "firefox", 128, "macOS 14 - Firefox 128 ESR"),
  macTemplate("14", "firefox", 150, "macOS 14 - Firefox 150"),
  macTemplate("14", "chrome", 147, "macOS 14 - Chrome 147"),
  macTemplate("14", "safari", 17, "macOS 14 - Safari 17"),
  macTemplate("15", "edge", 147, "macOS 15 - Edge 147"),
  macTemplate("15", "firefox", 140, "macOS 15 - Firefox 140 ESR"),
  macTemplate("15", "firefox", 128, "macOS 15 - Firefox 128 ESR"),
  macTemplate("15", "firefox", 150, "macOS 15 - Firefox 150"),
  macTemplate("15", "chrome", 147, "macOS 15 - Chrome 147"),
  macTemplate("15", "safari", 18, "macOS 15 - Safari 18"),
  macTemplate("26", "edge", 147, "macOS 26 - Edge 147"),
  macTemplate("26", "firefox", 140, "macOS 26 - Firefox 140 ESR"),
  macTemplate("26", "firefox", 128, "macOS 26 - Firefox 128 ESR"),
  macTemplate("26", "firefox", 150, "macOS 26 - Firefox 150"),
  macTemplate("26", "chrome", 147, "macOS 26 - Chrome 147"),
  macTemplate("26", "safari", 26, "macOS 26 - Safari 26")
];

const LINUX_TEMPLATES = [
  linuxTemplate("linux", "edge", 147, "Linux - Edge 147"),
  linuxTemplate("linux", "firefox", 140, "Linux - Firefox 140 ESR"),
  linuxTemplate("linux", "firefox", 128, "Linux - Firefox 128 ESR"),
  linuxTemplate("linux", "firefox", 150, "Linux - Firefox 150"),
  linuxTemplate("linux", "chrome", 147, "Linux - Chrome 147"),
  linuxTemplate("fedora", "edge", 147, "Fedora Linux - Edge 147"),
  linuxTemplate("fedora", "firefox", 140, "Fedora Linux - Firefox 140 ESR"),
  linuxTemplate("fedora", "firefox", 128, "Fedora Linux - Firefox 128 ESR"),
  linuxTemplate("fedora", "firefox", 150, "Fedora Linux - Firefox 150"),
  linuxTemplate("fedora", "chrome", 147, "Fedora Linux - Chrome 147"),
  linuxTemplate("ubuntu", "edge", 147, "Ubuntu Linux - Edge 147"),
  linuxTemplate("ubuntu", "firefox", 140, "Ubuntu Linux - Firefox 140 ESR"),
  linuxTemplate("ubuntu", "firefox", 128, "Ubuntu Linux - Firefox 128 ESR"),
  linuxTemplate("ubuntu", "firefox", 150, "Ubuntu Linux - Firefox 150"),
  linuxTemplate("ubuntu", "chrome", 147, "Ubuntu Linux - Chrome 147")
];

const IOS_TEMPLATES = [
  iosTemplate(17, "chrome", "phone", "iOS 17 - Chrome 148 (Phone)"),
  iosTemplate(17, "chrome", "tablet", "iOS 17 - Chrome 148 (Tablet)"),
  iosTemplate(17, "safari", "phone", "iOS 17 - Safari 17 (iPhone)"),
  iosTemplate(17, "safari", "tablet", "iOS 17 - Safari 17 (iPad)"),
  iosTemplate(18, "chrome", "phone", "iOS 18 - Chrome 148 (Phone)"),
  iosTemplate(18, "chrome", "tablet", "iOS 18 - Chrome 148 (Tablet)"),
  iosTemplate(18, "safari", "phone", "iOS 18 - Safari 18 (iPhone)"),
  iosTemplate(18, "safari", "tablet", "iOS 18 - Safari 18 (iPad)"),
  iosTemplate(26, "chrome", "phone", "iOS 26 - Chrome 148 (Phone)"),
  iosTemplate(26, "chrome", "tablet", "iOS 26 - Chrome 148 (Tablet)"),
  iosTemplate(26, "safari", "phone", "iOS 26 - Safari 26 (iPhone)"),
  iosTemplate(26, "safari", "tablet", "iOS 26 - Safari 26 (iPad)")
];

const ANDROID_TEMPLATES = [
  androidTemplate(11, "edge", "phone", "Android 11 - Edge 147 (Phone)"),
  androidTemplate(11, "firefox", "phone", "Android 11 - Firefox 150 (Phone)"),
  androidTemplate(11, "firefox", "tablet", "Android 11 - Firefox 150 (Tablet)"),
  androidTemplate(11, "chrome", "phone", "Android 11 - Chrome 147 (Phone)"),
  androidTemplate(11, "chrome", "tablet", "Android 11 - Chrome 147 (Tablet)"),
  androidTemplate(12, "edge", "phone", "Android 12 - Edge 147 (Phone)"),
  androidTemplate(12, "firefox", "phone", "Android 12 - Firefox 150 (Phone)"),
  androidTemplate(12, "firefox", "tablet", "Android 12 - Firefox 150 (Tablet)"),
  androidTemplate(12, "chrome", "phone", "Android 12 - Chrome 147 (Phone)"),
  androidTemplate(12, "chrome", "tablet", "Android 12 - Chrome 147 (Tablet)"),
  androidTemplate(13, "edge", "phone", "Android 13 - Edge 147 (Phone)"),
  androidTemplate(13, "firefox", "phone", "Android 13 - Firefox 150 (Phone)"),
  androidTemplate(13, "firefox", "tablet", "Android 13 - Firefox 150 (Tablet)"),
  androidTemplate(13, "chrome", "phone", "Android 13 - Chrome 147 (Phone)"),
  androidTemplate(13, "chrome", "tablet", "Android 13 - Chrome 147 (Tablet)"),
  androidTemplate(14, "edge", "phone", "Android 14 - Edge 147 (Phone)"),
  androidTemplate(14, "firefox", "phone", "Android 14 - Firefox 150 (Phone)"),
  androidTemplate(14, "firefox", "tablet", "Android 14 - Firefox 150 (Tablet)"),
  androidTemplate(14, "chrome", "phone", "Android 14 - Chrome 147 (Phone)"),
  androidTemplate(14, "chrome", "tablet", "Android 14 - Chrome 147 (Tablet)")
];

export const IDENTITY_TEMPLATES = [
  ...WINDOWS_TEMPLATES,
  ...MAC_TEMPLATES,
  ...LINUX_TEMPLATES,
  ...IOS_TEMPLATES,
  ...ANDROID_TEMPLATES
];
