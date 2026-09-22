import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { useLocale } from "@/stores/locale";
import enJSON from "./locale/en.json";
import faJSON from "./locale/fa.json";

const initialLocale = useLocale.getState().locale ?? "en";
if (typeof document !== "undefined") {
  document.documentElement.dir = initialLocale === "fa" ? "rtl" : "ltr";
  document.documentElement.lang = initialLocale;
}

i18n.use(initReactI18next).init({
  fallbackLng: "en",
  interpolation: {
    escapeValue: false,
  },
  lng: initialLocale,
  resources: {
    en: { translation: enJSON },
    fa: { translation: faJSON },
  },
});

useLocale.subscribe(({ locale }) => {
  if (typeof document !== "undefined") {
    document.documentElement.dir = locale === "fa" ? "rtl" : "ltr";
    document.documentElement.lang = locale;
  }
  i18n.changeLanguage(locale);
});
