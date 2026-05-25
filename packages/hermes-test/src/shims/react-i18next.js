// Built-in react-i18next shim for hermes-test.
// Identity translation: t('key') returns 'key'.
// Covers useTranslation, withTranslation, Trans, initReactI18next.

var t = function(key) { return key; };
var i18n = {
  language: 'en',
  changeLanguage: function() { return Promise.resolve(); },
  use: function() { return i18n; },
  init: function() { return Promise.resolve(); },
};

module.exports = {
  useTranslation: function() { return { t: t, i18n: i18n, ready: true }; },
  withTranslation: function() { return function(component) { return component; }; },
  Trans: function(props) { return props.children || null; },
  Translation: function(props) { return props.children(t, { i18n: i18n }); },
  initReactI18next: { type: '3rdParty', init: function() {} },
  I18nextProvider: function(props) { return props.children; },
};
