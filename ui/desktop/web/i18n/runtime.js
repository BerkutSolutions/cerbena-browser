export function createI18n(dictionaries, locale) {
  let currentLocale = locale;

  if (typeof document !== "undefined") {
    document.documentElement.lang = currentLocale;
  }

  function t(key) {
    const value = dictionaries[currentLocale]?.[key];
    if (typeof value === "string") {
      return value;
    }
    return key;
  }

  function setLocale(nextLocale) {
    if (!dictionaries[nextLocale]) {
      throw new Error(`Unsupported locale: ${nextLocale}`);
    }
    currentLocale = nextLocale;
    localStorage.setItem("launcher.locale", nextLocale);
    if (typeof document !== "undefined") {
      document.documentElement.lang = nextLocale;
    }
  }

  function getLocale() {
    return currentLocale;
  }

  return { t, setLocale, getLocale };
}

export async function loadDictionaries() {
  const [en, ru] = await Promise.all([
    fetch(new URL("./en/common.json", import.meta.url)).then((r) => r.json()),
    fetch(new URL("./ru/common.json", import.meta.url)).then((r) => r.json())
  ]);

  return { en, ru };
}
