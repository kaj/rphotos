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
  function prepare_map(cb) {
    let h = d.querySelector('head');
    let m = d.querySelector('.meta') || d.querySelector('main');
    m.insertAdjacentHTML('beforeend', '<div id="map"></div>');
    var map = d.getElementById('map');
    map.style.height = 3 * map.clientWidth / 4 + "px";
    var slink = d.createElement('script');
    slink.type = 'text/javascript';
    slink.src = '/static/l131/leaflet.js';
    slink.async = 'async';
    slink.onload = () => {
      var map = L.map('map', {'scrollWheelZoom': false});
      L.tileLayer('//{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
	attribution: '© <a href="http://osm.org/copyright">OpenStreetMap</a> contributors'
      }).addTo(map);
      cb(map);
    }
    h.append(slink);
    var csslink = d.createElement('link');
    csslink.rel = 'stylesheet';
    csslink.href = '/static/l131/leaflet.css';
    h.append(csslink);
  }
  let details = d.querySelector('.details');
  let pos = details && details.dataset.position
  if (pos) {
    prepare_map((map) => {
      let pos = JSON.parse(pos);
      map.setView(pos, 16);
      L.marker(pos).addTo(map);
    })
  }
  let group = d.querySelector('.group');
  let poss = (details && details.dataset.positions) || (group && group.dataset.positions);
  if (poss) {
    prepare_map((map) => {
      let pos = JSON.parse(poss)
      map.fitBounds(L.polyline(pos).getBounds())
      pos.forEach(p => L.marker(p).addTo(map));
    })
  }
})(document)
