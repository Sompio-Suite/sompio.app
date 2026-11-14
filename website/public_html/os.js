// Download page: shows the download and install steps for the visitor's system, with tabs for the
// others. Without this script, every system's section simply shows.
(function () {
  var ua = navigator.userAgent;
  var guess = /Mac OS X|Macintosh/.test(ua) && !/iPhone|iPad/.test(ua) ? "mac" : /Linux/.test(ua) && !/Android/.test(ua) ? "linux" : "windows";
  var tabs = document.querySelectorAll(".os-tab");
  if (!tabs.length) return;
  var offered = Array.prototype.map.call(tabs, function (t) { return t.getAttribute("data-os"); });
  var fromLink = (location.hash.match(/^#for-(windows|mac|linux)$/) || [])[1];
  function show(os) {
    if (offered.indexOf(os) < 0) os = "windows";
    document.querySelectorAll(".os-panel").forEach(function (p) { p.hidden = p.getAttribute("data-os") !== os; });
    tabs.forEach(function (t) {
      var on = t.getAttribute("data-os") === os;
      t.classList.toggle("on", on);
      t.setAttribute("aria-selected", on ? "true" : "false");
    });
  }
  tabs.forEach(function (t) {
    t.addEventListener("click", function (e) {
      e.preventDefault();
      show(t.getAttribute("data-os"));
      history.replaceState(null, "", "#for-" + t.getAttribute("data-os"));
    });
  });
  show(fromLink || guess);
})();
