(function(d) {
  let f = d.querySelector('footer');
  f.insertAdjacentHTML(
    'afterbegin',
    '<p><a href="#help" title="Help" accesskey="?">?</a></p>')
  f.querySelector('[href="#help"]').addEventListener('click', e => {
    if (d.getElementById('help') == null) {
      f.insertAdjacentHTML(
        'beforebegin',
        '<div id="help"><h2>Key bindings</h2>' +
        [].map.call(
          d.querySelectorAll('[accesskey]'),
          e => e.accessKeyLabel + ": " + (e.title || e.innerText)).join('<br/>') +
        '</div>');
    }
    return true;
  });
  let i = d.querySelector('.details .item');
  if (i) {
    i.addEventListener('click', e => { i.classList.toggle('zoom') });
  }
  let details = d.querySelector('.details');
  let pos = details && details.dataset.position
  if (pos) {
    function initmap(pos) {
      var map = document.getElementById('map');
      map.style.height = 3 * map.clientWidth / 4 + "px";
      var map = L.map('map').setView(pos, 16);
      L.tileLayer('//{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
	attribution: '© <a href="http://osm.org/copyright">OpenStreetMap</a> contributors'
      }).addTo(map);
      L.marker(pos).addTo(map);
    }
    let h = d.querySelector('head');
    let m = d.querySelector('.meta');
    m.insertAdjacentHTML('beforeend', '<div id="map"></div>');
    var slink = document.createElement('script');
    slink.type = 'text/javascript';
    slink.src = 'https://rasmus.krats.se/static/leaflet077c/leaflet.js';
    slink.async = 'async';
    slink.onload = () => initmap(JSON.parse(pos));
    h.append(slink);
    var csslink = document.createElement('link');
    csslink.rel = 'stylesheet';
    csslink.href = 'https://rasmus.krats.se/static/leaflet077c/leaflet.css';
    h.append(csslink);
  }
})(document)
