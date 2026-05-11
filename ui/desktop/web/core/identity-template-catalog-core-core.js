import {
  androidCore,
  desktopLocale,
  makeTemplate,
  iosCore,
  linuxCore,
  macCore,
  windowsCore
} from "./identity-template-catalog-core-primitives.js";

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


export { windowsTemplate, macTemplate, linuxTemplate, iosTemplate, androidTemplate };
