(function(d) {
  if (d.querySelector('main form.search')) {
    d.querySelector('header form.search').remove();
    d.querySelector('main form.search input#s_q').focus();
  }
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
  var map;
  function resize_map() {
    var me = d.getElementById('map');
    if (me) {
      me.style.height = 4 * me.clientWidth / 5 + "px";
    }
    if (map) {
      map.invalidateSize(false);
    }
  }
  window.addEventListener('resize', resize_map);
  let i = d.querySelector('.details .item');
  if (i) {
    i.addEventListener('click', e => {
      i.classList.toggle('zoom');
      resize_map();
    });
  }
  function prepare_map(cb) {
    let h = d.querySelector('head');
    var csslink = d.createElement('link');
    csslink.rel = 'stylesheet';
    csslink.href = '/static/l140/leaflet.css';
    h.append(csslink);
    let m = d.querySelector('.meta') || d.querySelector('main');
    m.insertAdjacentHTML('beforeend', '<div id="map"></div>');
    var slink = d.createElement('script');
    slink.type = 'text/javascript';
    slink.src = '/static/l140/leaflet.js';
    slink.async = 'async';
    slink.onload = () => {
      map = L.map('map', {'scrollWheelZoom': false, 'trackResize': false});
      L.tileLayer('//{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
	attribution: '© <a href="http://osm.org/copyright">OpenStreetMap</a> contributors'
      }).addTo(map);
      resize_map();
      cb(map);
    }
    h.append(slink);
  }
  let details = d.querySelector('.details');
  let pos = details && details.dataset.position
  if (pos) {
    prepare_map((map) => {
      let p = JSON.parse(pos);
      map.setView(p, 16);
      L.marker(p).addTo(map);
    })
  }
  let group = d.querySelector('.group');
  let poss = (details && details.dataset.positions) || (group && group.dataset.positions);
  if (poss) {
    prepare_map((map) => {
      let h = d.querySelector('head');
      h.insertAdjacentHTML(
        'beforeend',
        '<link rel="stylesheet" href="/static/lm141/lmc.css">' +
        '<link rel="stylesheet" href="/static/lm141/lmc-default.css">'
      )
      var slink2 = d.createElement('script');
      slink2.type = 'text/javascript';
      slink2.src = '/static/lm141/lmc.js';
      h.append(slink2);
      slink2.onload = () => {
        let pos = JSON.parse(poss);
        var markers = L.markerClusterGroup({maxClusterRadius: 35});
        map.fitBounds(L.polyline(pos).getBounds());
        pos.forEach(p => {
          let n = p.pop();
          let m = L.marker(p, { title: n });
          m.bindPopup(`<a href="/img/${n}"><img src="/img/${n}-s.jpg"></a>`);
          markers.addLayer(m);
        });
        map.addLayer(markers);
      };
    })
  }

  (function(form) {
    form.classList.add('hidden');
    let sl = form.querySelector('label');
    sl.addEventListener('click', e => form.classList.remove('hidden'))
    let list = d.createElement('div');
    list.className = 'list';
    let tags = form.querySelector('div.refs');
    form.insertBefore(list, tags);
    let kindname = { 't': 'tag', 'p': 'person', 'l': 'place'}
    let input = form.querySelector('input[name=q]');
    input.autocomplete = "off";
    input.addEventListener('keyup', e => {
      let v = e.target.value;
      if (new Set(['ArrowUp', 'ArrowDown', 'Escape']).has(e.code)) {
	return;
      }
      if (v.length > 1) {
	let r = new XMLHttpRequest();
	r.onload = function() {
	  let t = JSON.parse(this.responseText);
	  list.innerHTML = '';
	  t.map(x => {
	    let a = d.createElement('a');
	    a.innerHTML = x.t + ' <small>(' + kindname[x.k] + ')</small>';
	    a.className='hit ' + x.k;
	    a.href = x.s;
	    a.onclick = function() {
	      let s = d.createElement('label');
	      s.innerHTML = x.t + ' <input type="checkbox" checked name="' + x.k +
		'" value="' + x.s + '">';
	      s.className = x.k;
	      tags.insertBefore(s, input);
	      list.innerHTML = '';
	      input.value = '';
	      input.focus();
	      return false;
	    }
	    list.appendChild(a)
	  })
	};
	r.open('GET', d.location.origin + '/ac?q=' + encodeURIComponent(v));
	r.send(null);
      } else {
	list.innerHTML = '';
      }
    })
    form.addEventListener('keyup', e => {
      let t = e.target;
      switch(e.code) {
      case 'ArrowUp':
	(t.parentNode == list && t.previousSibling || list.querySelector('a:last-child')).focus();
	break;
      case 'ArrowDown':
	(t.parentNode == list && t.nextSibling || list.querySelector('a:first-child')).focus();
	break;
      case 'Escape':
	if (list.hasChildNodes()) {
	  input.focus();
	  list.innerHTML = '';
	} else {
	  form.classList.add('hidden');
	}
	break;
      default:
	return true;
      };
      e.preventDefault();
      e.stopPropagation();
      return false;
    });
    //form.querySelector('.help .js').innerHTML = 'Du kan begränsa din sökning till de taggar som föreslås.';
  })(d.querySelector('form.search'));
})(document)
