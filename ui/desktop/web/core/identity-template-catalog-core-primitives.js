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


export {
  slugify,
  desktopWindow,
  mobileWindow,
  desktopLocale,
  makeTemplate,
  windowsCore,
  macCore,
  linuxCore,
  iosCore,
  androidCore
};
