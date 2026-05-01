function slugify(value) {
  return String(value ?? "")
    .toLowerCase()
    .replaceAll(".", "_")
    .replaceAll("(", "")
    .replaceAll(")", "")
    .replaceAll("+", "plus")
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
}

function desktopWindow(screen, chromeLike = true) {
  const outerWidth = Math.max(1024, screen.avail_width);
  const outerHeight = Math.max(720, screen.avail_height);
  const innerWidth = Math.max(980, outerWidth - (chromeLike ? 38 : 34));
  const innerHeight = Math.max(660, outerHeight - (chromeLike ? 84 : 76));
  return {
    outer_width: outerWidth,
    outer_height: outerHeight,
    inner_width: innerWidth,
    inner_height: innerHeight,
    screen_x: 0,
    screen_y: 0
  };
}

function mobileWindow(width, height) {
  return {
    outer_width: width,
    outer_height: height,
    inner_width: width,
    inner_height: Math.max(320, height - 88),
    screen_x: 0,
    screen_y: 0
  };
}

function desktopLocale(language, timezone, offsetMinutes, latitude, longitude, accuracyMeters) {
  const baseLanguage = language.includes("-") ? language.split("-")[0] : language;
  return {
    locale: {
      navigator_language: language,
      languages: [language, baseLanguage],
      do_not_track: "1",
      timezone_iana: timezone,
      timezone_offset_minutes: offsetMinutes
    },
    geo: {
      latitude,
      longitude,
      accuracy_meters: accuracyMeters
    }
  };
}

function makeTemplate({
  label,
  platformFamily,
  autoPlatform,
  core,
  hardware,
  screen,
  locale,
  geo,
  webgl,
  fonts,
  battery
}) {
  return {
    key: slugify(label),
    label,
    platformFamily,
    autoPlatform,
    core,
    hardware,
    screen,
    window: hardware.max_touch_points > 2
      ? mobileWindow(screen.width, screen.height)
      : desktopWindow(screen, core.brand !== "Firefox" && core.brand !== "Internet Explorer" && core.brand !== "Safari"),
    locale,
    geo,
    webgl,
    fonts,
    battery
  };
}

function windowsCore(osVersion, browser, browserVersion, firefoxVersion = browserVersion) {
  const osToken = {
    "7": "Windows NT 6.1",
    "8": "Windows NT 6.2",
    "8.1": "Windows NT 6.3",
    "10": "Windows NT 10.0"
  }[osVersion] ?? "Windows NT 10.0";
  if (browser === "edge") {
    return {
      user_agent: `Mozilla/5.0 (${osToken}; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${browserVersion}.0.0.0 Safari/537.36 Edg/${browserVersion}.0.0.0`,
      platform: "Win32",
      platform_version: osVersion === "10" ? "10.0" : osToken.replace("Windows NT ", ""),
      brand: "Microsoft Edge",
      brand_version: String(browserVersion),
      vendor: "Google Inc.",
      vendor_sub: "",
      product_sub: "20030107"
    };
  }
  if (browser === "chrome") {
    return {
      user_agent: `Mozilla/5.0 (${osToken}; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${browserVersion}.0.0.0 Safari/537.36`,
      platform: "Win32",
      platform_version: osVersion === "10" ? "10.0" : osToken.replace("Windows NT ", ""),
      brand: "Chromium",
      brand_version: String(browserVersion),
      vendor: "Google Inc.",
      vendor_sub: "",
      product_sub: "20030107"
    };
  }
  if (browser === "ie") {
    return {
      user_agent: `Mozilla/5.0 (${osToken}; Trident/7.0; rv:11.0) like Gecko`,
      platform: "Win32",
      platform_version: osVersion === "10" ? "10.0" : osToken.replace("Windows NT ", ""),
      brand: "Internet Explorer",
      brand_version: "11",
      vendor: "",
      vendor_sub: "",
      product_sub: "20100101"
    };
  }
  return {
    user_agent: `Mozilla/5.0 (${osToken}; Win64; x64; rv:${firefoxVersion}.0) Gecko/20100101 Firefox/${firefoxVersion}.0`,
    platform: "Win32",
    platform_version: osVersion === "10" ? "10.0" : osToken.replace("Windows NT ", ""),
    brand: "Firefox",
    brand_version: String(firefoxVersion),
    vendor: "",
    vendor_sub: "",
    product_sub: "20100101"
  };
}

function macCore(osVersion, browser, browserVersion) {
  const macToken = String(osVersion).replaceAll(".", "_");
  if (browser === "safari") {
    return {
      user_agent: `Mozilla/5.0 (Macintosh; Intel Mac OS X ${macToken}) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/${browserVersion}.0 Safari/605.1.15`,
      platform: "MacIntel",
      platform_version: osVersion,
      brand: "Safari",
      brand_version: String(browserVersion),
      vendor: "Apple Computer, Inc.",
      vendor_sub: "",
      product_sub: "20030107"
    };
  }
  if (browser === "edge") {
    return {
      user_agent: `Mozilla/5.0 (Macintosh; Intel Mac OS X ${macToken}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${browserVersion}.0.0.0 Safari/537.36 Edg/${browserVersion}.0.0.0`,
      platform: "MacIntel",
      platform_version: osVersion,
      brand: "Microsoft Edge",
      brand_version: String(browserVersion),
      vendor: "Google Inc.",
      vendor_sub: "",
      product_sub: "20030107"
    };
  }
  if (browser === "chrome") {
    return {
      user_agent: `Mozilla/5.0 (Macintosh; Intel Mac OS X ${macToken}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${browserVersion}.0.0.0 Safari/537.36`,
      platform: "MacIntel",
      platform_version: osVersion,
      brand: "Chromium",
      brand_version: String(browserVersion),
      vendor: "Google Inc.",
      vendor_sub: "",
      product_sub: "20030107"
    };
  }
  return {
    user_agent: `Mozilla/5.0 (Macintosh; Intel Mac OS X ${macToken}; rv:${browserVersion}.0) Gecko/20100101 Firefox/${browserVersion}.0`,
    platform: "MacIntel",
    platform_version: osVersion,
    brand: "Firefox",
    brand_version: String(browserVersion),
    vendor: "",
    vendor_sub: "",
    product_sub: "20100101"
  };
}

function linuxCore(distro, browser, browserVersion) {
  const distroToken = distro === "ubuntu" ? "X11; Ubuntu; Linux x86_64" : distro === "fedora" ? "X11; Fedora; Linux x86_64" : "X11; Linux x86_64";
  const platformVersion = distro === "ubuntu" ? "6.8" : distro === "fedora" ? "6.12" : "6.6";
  if (browser === "edge") {
    return {
      user_agent: `Mozilla/5.0 (${distroToken}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${browserVersion}.0.0.0 Safari/537.36 Edg/${browserVersion}.0.0.0`,
      platform: "Linux x86_64",
      platform_version: platformVersion,
      brand: "Microsoft Edge",
      brand_version: String(browserVersion),
      vendor: "Google Inc.",
      vendor_sub: "",
      product_sub: "20030107"
    };
  }
  if (browser === "chrome") {
    return {
      user_agent: `Mozilla/5.0 (${distroToken}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${browserVersion}.0.0.0 Safari/537.36`,
      platform: "Linux x86_64",
      platform_version: platformVersion,
      brand: "Chromium",
      brand_version: String(browserVersion),
      vendor: "Google Inc.",
      vendor_sub: "",
      product_sub: "20030107"
    };
  }
  return {
    user_agent: `Mozilla/5.0 (${distroToken}; rv:${browserVersion}.0) Gecko/20100101 Firefox/${browserVersion}.0`,
    platform: "Linux x86_64",
    platform_version: platformVersion,
    brand: "Firefox",
    brand_version: String(browserVersion),
    vendor: "",
    vendor_sub: "",
    product_sub: "20100101"
  };
}

function iosCore(version, browser, deviceType) {
  const iosToken = String(version).replaceAll(".", "_");
  const deviceToken = deviceType === "tablet" ? "iPad; CPU OS" : "iPhone; CPU iPhone OS";
  const platform = deviceType === "tablet" ? "iPad" : "iPhone";
  if (browser === "chrome") {
    return {
      user_agent: `Mozilla/5.0 (${deviceToken} ${iosToken} like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/148.0.0.0 Mobile/15E148 Safari/604.1`,
      platform,
      platform_version: String(version),
      brand: "Chrome iOS",
      brand_version: "148",
      vendor: "Apple Computer, Inc.",
      vendor_sub: "",
      product_sub: "20030107"
    };
  }
  return {
    user_agent: `Mozilla/5.0 (${deviceToken} ${iosToken} like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/${version}.0 Mobile/15E148 Safari/604.1`,
    platform,
    platform_version: String(version),
    brand: "Safari",
    brand_version: String(version),
    vendor: "Apple Computer, Inc.",
    vendor_sub: "",
    product_sub: "20030107"
  };
}

function androidCore(version, browser, deviceType) {
  const deviceName = deviceType === "tablet"
    ? (version >= 13 ? "SM-X616B" : "SM-X610")
    : (version >= 13 ? "Pixel 8 Pro" : "Pixel 7");
  const mobileToken = deviceType === "tablet" ? "" : " Mobile";
  if (browser === "edge") {
    return {
      user_agent: `Mozilla/5.0 (Linux; Android ${version}; ${deviceName}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0${mobileToken} Safari/537.36 EdgA/147.0.0.0`,
      platform: "Linux armv8l",
      platform_version: String(version),
      brand: "Microsoft Edge",
      brand_version: "147",
      vendor: "Google Inc.",
      vendor_sub: "",
      product_sub: "20030107"
    };
  }
  if (browser === "chrome") {
    return {
      user_agent: `Mozilla/5.0 (Linux; Android ${version}; ${deviceName}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0${mobileToken} Safari/537.36`,
      platform: "Linux armv8l",
      platform_version: String(version),
      brand: "Chromium",
      brand_version: "147",
      vendor: "Google Inc.",
      vendor_sub: "",
      product_sub: "20030107"
    };
  }
  const firefoxDeviceToken = deviceType === "tablet" ? "Tablet" : "Mobile";
  return {
    user_agent: `Mozilla/5.0 (Android ${version}; ${firefoxDeviceToken}; rv:150.0) Gecko/150.0 Firefox/150.0`,
    platform: "Linux armv8l",
    platform_version: String(version),
    brand: "Firefox",
    brand_version: "150",
    vendor: "",
    vendor_sub: "",
    product_sub: "20100101"
  };
}

function windowsTemplate(osVersion, browser, browserVersion, label) {
  const screenByVersion = {
    "7": { width: 1440, height: 900, device_pixel_ratio: 1, avail_width: 1440, avail_height: 860, color_depth: 24 },
    "8": { width: 1600, height: 900, device_pixel_ratio: 1, avail_width: 1600, avail_height: 860, color_depth: 24 },
    "8.1": { width: 1366, height: 768, device_pixel_ratio: 1, avail_width: 1366, avail_height: 728, color_depth: 24 },
    "10": { width: 1920, height: 1080, device_pixel_ratio: 1, avail_width: 1920, avail_height: 1040, color_depth: 24 }
  }[osVersion];
  const localeByVersion = {
    "7": desktopLocale("en-US", "America/New_York", 300, 40.7128, -74.006, 22),
    "8": desktopLocale("ru-RU", "Europe/Moscow", -180, 55.7558, 37.6173, 26),
    "8.1": desktopLocale("de-DE", "Europe/Berlin", -60, 52.52, 13.405, 21),
    "10": desktopLocale("en-GB", "Europe/London", 0, 51.5072, -0.1276, 18)
  }[osVersion];
  return makeTemplate({
    label,
    platformFamily: "windows",
    autoPlatform: "windows",
    core: windowsCore(osVersion, browser, browserVersion),
    hardware: {
      cpu_threads: osVersion === "10" ? 8 : 4,
      max_touch_points: 0,
      device_memory_gb: osVersion === "10" ? 16 : 8
    },
    screen: screenByVersion,
    locale: localeByVersion.locale,
    geo: localeByVersion.geo,
    webgl: {
      vendor: browser === "firefox" ? "Mozilla" : "Google Inc. (Intel)",
      renderer: osVersion === "10"
        ? "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0)"
        : "ANGLE (Intel, Intel(R) HD Graphics 520 Direct3D11 vs_5_0 ps_5_0)",
      params_json: "{\"maxTextureSize\":16384}"
    },
    fonts: ["Arial", "Segoe UI", "Calibri", "Tahoma"],
    battery: { charging: osVersion === "10", level: osVersion === "10" ? 0.84 : 0.63 }
  });
}

function macTemplate(osVersion, browser, browserVersion, label) {
  const localeByVersion = {
    "14": desktopLocale("en-US", "America/Los_Angeles", 480, 37.7749, -122.4194, 18),
    "15": desktopLocale("en-US", "America/Chicago", 360, 41.8781, -87.6298, 16),
    "26": desktopLocale("en-US", "America/Toronto", 300, 43.6532, -79.3832, 19)
  }[String(osVersion)];
  return makeTemplate({
    label,
    platformFamily: "macos",
    autoPlatform: "macos",
    core: macCore(String(osVersion), browser, browserVersion),
    hardware: { cpu_threads: 8, max_touch_points: 0, device_memory_gb: 16 },
    screen: { width: 1440, height: 900, device_pixel_ratio: 2, avail_width: 1440, avail_height: 860, color_depth: 24 },
    locale: localeByVersion.locale,
    geo: localeByVersion.geo,
    webgl: {
      vendor: browser === "firefox" ? "Mozilla" : "Apple",
      renderer: "Apple MTL Renderer",
      params_json: "{\"maxTextureSize\":16384}"
    },
    fonts: ["SF Pro Text", "Helvetica Neue", "Arial", "Menlo"],
    battery: { charging: true, level: 0.9 }
  });
}

function linuxTemplate(distro, browser, browserVersion, label) {
  const locale = distro === "ubuntu"
    ? desktopLocale("en-GB", "Europe/London", 0, 51.5072, -0.1276, 18)
    : distro === "fedora"
      ? desktopLocale("en-US", "America/Chicago", 360, 41.8781, -87.6298, 20)
      : desktopLocale("fr-FR", "Europe/Paris", -60, 48.8566, 2.3522, 21);
  const screen = distro === "ubuntu"
    ? { width: 2560, height: 1440, device_pixel_ratio: 1, avail_width: 2560, avail_height: 1400, color_depth: 24 }
    : { width: 1920, height: 1080, device_pixel_ratio: 1, avail_width: 1920, avail_height: 1040, color_depth: 24 };
  return makeTemplate({
    label,
    platformFamily: "linux",
    autoPlatform: "linux",
    core: linuxCore(distro, browser, browserVersion),
    hardware: { cpu_threads: 8, max_touch_points: 0, device_memory_gb: 16 },
    screen,
    locale: locale.locale,
    geo: locale.geo,
    webgl: {
      vendor: browser === "firefox" ? "Mozilla" : "Google Inc. (Mesa)",
      renderer: distro === "ubuntu" ? "Mesa Intel(R) Iris(R) Xe Graphics" : "Mesa AMD Radeon Graphics",
      params_json: "{\"antialias\":true}"
    },
    fonts: distro === "ubuntu"
      ? ["Ubuntu", "Noto Sans", "Liberation Sans", "DejaVu Sans"]
      : ["Noto Sans", "Liberation Sans", "DejaVu Sans", "Cantarell"],
    battery: { charging: distro !== "fedora", level: distro === "fedora" ? 0.58 : 0.79 }
  });
}

function iosTemplate(version, browser, deviceType, label) {
  const isTablet = deviceType === "tablet";
  const locale = desktopLocale("en-US", "America/Los_Angeles", 480, 34.0522, -118.2437, 10);
  return makeTemplate({
    label,
    platformFamily: "ios",
    autoPlatform: "ios",
    core: iosCore(version, browser, deviceType),
    hardware: {
      cpu_threads: isTablet ? 8 : 6,
      max_touch_points: 5,
      device_memory_gb: isTablet ? 8 : 6
    },
    screen: isTablet
      ? { width: 1024, height: 1366, device_pixel_ratio: 2, avail_width: 1024, avail_height: 1320, color_depth: 24 }
      : { width: 430, height: 932, device_pixel_ratio: 3, avail_width: 430, avail_height: 900, color_depth: 24 },
    locale: locale.locale,
    geo: locale.geo,
    webgl: {
      vendor: "Apple",
      renderer: isTablet ? "Apple M2 GPU" : "Apple A17 GPU",
      params_json: "{\"maxTextureSize\":16384}"
    },
    fonts: ["SF Pro Text", "SF Pro Display", "Helvetica Neue", "Arial"],
    battery: { charging: false, level: isTablet ? 0.73 : 0.56 }
  });
}

function androidTemplate(version, browser, deviceType, label) {
  const isTablet = deviceType === "tablet";
  const locale = desktopLocale("en-US", "America/Los_Angeles", 480, 34.0522, -118.2437, 12);
  return makeTemplate({
    label,
    platformFamily: "android",
    autoPlatform: "android",
    core: androidCore(version, browser, deviceType),
    hardware: {
      cpu_threads: isTablet ? 8 : 8,
      max_touch_points: 10,
      device_memory_gb: isTablet ? 8 : 8
    },
    screen: isTablet
      ? { width: 800, height: 1280, device_pixel_ratio: 2, avail_width: 800, avail_height: 1232, color_depth: 24 }
      : { width: 412, height: 915, device_pixel_ratio: 2.625, avail_width: 412, avail_height: 884, color_depth: 24 },
    locale: locale.locale,
    geo: locale.geo,
    webgl: {
      vendor: browser === "firefox" ? "Mozilla" : "Google Inc. (Qualcomm)",
      renderer: isTablet ? "ANGLE (ARM, Mali-G715 OpenGL ES 3.2)" : "ANGLE (Qualcomm, Adreno 740 OpenGL ES 3.2)",
      params_json: "{\"maxTextureSize\":16384}"
    },
    fonts: ["Roboto", "Noto Sans", "Google Sans"],
    battery: { charging: false, level: isTablet ? 0.68 : 0.49 }
  });
}

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
