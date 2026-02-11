import i18n from "i18next"
import { initReactI18next } from "react-i18next"
import en from "./en.json"
import zhTW from "./zh-TW.json"
import ja from "./ja.json"

i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    "zh-TW": { translation: zhTW },
    ja: { translation: ja },
  },
  lng: localStorage.getItem("i18nextLng") || "en",
  fallbackLng: "en",
  interpolation: {
    escapeValue: false,
  },
})

i18n.on("languageChanged", (lng) => {
  localStorage.setItem("i18nextLng", lng)
})

export default i18n
