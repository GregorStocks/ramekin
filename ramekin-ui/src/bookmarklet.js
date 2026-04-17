(function () {
  var s = document.createElement("script");
  s.src =
    "__ORIGIN__/capture.js?token=__TOKEN__&api=__API__&external=__EXTERNAL__&t=" +
    Date.now();
  s.crossOrigin = "anonymous";
  document.body.appendChild(s);
})();
