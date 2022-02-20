// Admin functionality for rphotos
(function (d) {
    var details = d.querySelector('main.details'), p;
    if (!details) {
        return; // Admin is for single image only
    }

    function rotate(event) {
        var imgid = details.dataset.imgid;
        var angle = event.target.dataset.angle;
        var r = new XMLHttpRequest();
        d.body.classList.add('busy');
        r.open('POST', '/adm/rotate');
        r.onload = function() {
            if (r.status === 200) {
                // Yay!
                d.location.reload(true);
            } else {
                alert("Rotating failed: " + r.status);
            }
            d.body.classList.remove('busy');
        }
        r.onerror = function() {
            alert("Rotating failed.");
            d.body.classList.remove('busy');
        }
        r.ontimeout = function() {
            alert("Rotating timed out");
            d.body.classList.remove('busy');
        }
        r.setRequestHeader("Content-type", "application/x-www-form-urlencoded");
        r.send("angle=" + angle + "&image=" + imgid)
    }

    function makeform(category) {
        let oldform = p.querySelector('form');
        if (oldform) {
            oldform.remove();
        }
        var f = d.createElement("form");
        f.className = "admin " + category;
        f.action = "/adm/" + category;
        f.method = "post";
        var i = d.createElement("input");
        i.type="hidden";
        i.name="image";
        i.value = details.dataset.imgid;
        f.appendChild(i);
        return f;
    }
    function disable_one(one) {
        p.querySelectorAll('button').forEach(function(b) {
            b.disabled = (b === one);
        })
    }
    function tag_form(event, category) {
        disable_one(event.target);
        var f = makeform(category);
        var l = d.createElement("label");
        l.innerHTML = event.target.title;
        f.appendChild(l);
        var i = d.createElement("input");
        i.type = "text";
        i.autocomplete="off";
        i.tabindex="1";
        i.name = category;
        i.id = category+'name';
        l.htmlFor = i.id;
        var list = d.createElement("div");
        list.className = "completions";
        i.addEventListener('input', e => {
            let i = e.target, v = i.value;
            if (v.length > 0) {
		let r = new XMLHttpRequest();
		r.onload = function() {
                    let t = JSON.parse(this.responseText);
                    list.style.top = i.offsetTop + i.offsetHeight + "px"
                    list.style.left = i.offsetLeft + "px"
                    list.innerHTML = '';
                    t.map(x => {
			let a = d.createElement('a');
			a.innerHTML = x;
			a.tabIndex = 2;
			a.href = "#";
			a.onclick = function(e) {
                            i.value = x;
                            list.innerHTML = '';
                            i.focus();
                            e.preventDefault();
                            e.stopPropagation();
                            return true;
			}
			list.appendChild(a)
                    })
		};
		r.open('GET', d.location.origin + '/ac/' + category + '?q=' + encodeURIComponent(v));
		r.send(null);
            } else {
		list.innerHTML = '';
            }
	});
        f.appendChild(i);
        f.appendChild(list);
        f.addEventListener('keydown', e => {
            if (!list.innerHTML) {
                if (e.code === 'Escape') {
                    if (i.value) {
                        i.value = '';
                        i.focus();
                    } else { // close form
                        e.target.closest('form').remove();
                        event.target.disabled = false;
                        event.target.focus();
                    }
                    e.preventDefault();
                    e.stopPropagation();
                    return false;
                }
                return;
            }
            let t = e.target;
            switch(e.code) {
            case 'ArrowUp':
                (t.parentNode == list && t.previousSibling || list.querySelector('a:last-child')).focus();
                break;
            case 'ArrowDown':
                (t.parentNode == list && t.nextSibling || list.querySelector('a:first-child')).focus();
                break;
            case 'Escape':
                list.innerHTML = '';
                i.focus();
                break;
            default:
                return true;
            };
            e.preventDefault();
            e.stopPropagation();
            return false;
        });
        let s = d.createElement("button");
        s.innerHTML = "Ok";
        s.type = "submit";
        f.appendChild(s);
        let c = d.createElement("button");
        c.innerHTML = "&#x1f5d9;";
        c.className = 'close';
        c.title = 'close';
        c.onclick = e => {
            e.target.closest('form').remove();
            event.target.disabled = false; // The old event creating this form
            event.target.focus();
        };
        f.appendChild(c);
        p.append(f);
        i.focus();
    }

    function grade_form(event) {
        disable_one(event.target);
        var grade = details.dataset.grade;
        var f = makeform("grade");
        var l = d.createElement("label");
        l.innerHTML = event.target.title;
        f.appendChild(l);
        var i = d.createElement("input");
        i.type="range";
        i.name="grade";
        if (grade) {
            i.value=grade;
        }
        i.min=0;
        i.max=100;
        f.appendChild(i);
        let s = d.createElement("button");
        s.innerHTML = "Ok";
        s.type = "submit";
        f.appendChild(s);
        let c = d.createElement("button");
        c.innerHTML = "&#x1f5d9;";
        c.className = 'close';
        c.title = 'close';
        c.onclick = e => {
            e.target.closest('form').remove();
            event.target.disabled = false; // The old event creating this form
            event.target.focus();
        };
        f.appendChild(c);
        f.addEventListener('keydown', e => {
            switch(e.code) {
            case 'Escape':
                e.target.closest('form').remove();
                event.target.disabled = false;
                event.target.focus();
                break;
            case 'Enter':
                f.submit();
                break;
            default:
                return true;
            };
            e.preventDefault();
            e.stopPropagation();
            return false;
        });
        p.append(f);
        i.focus();
    }

    function location_form(event) {
        disable_one(event.target);
        var position = details.dataset.position || localStorage.getItem('lastpos');
        var f = makeform("locate");

        var lat = d.createElement("input");
        lat.type="hidden";
        lat.name="lat";
        f.appendChild(lat);
        var lng = d.createElement("input");
        lng.type="hidden";
        lng.name="lng";
        f.appendChild(lng);

        let h = d.querySelector('head');
        var csslink = d.createElement('link');
        csslink.rel = 'stylesheet';
        csslink.href = '/static/l140/leaflet.css';
        h.append(csslink);
        f.insertAdjacentHTML('beforeend', '<div id="amap"></div>');
        var slink = d.createElement('script');
        slink.type = 'text/javascript';
        slink.src = '/static/l140/leaflet.js';
        slink.async = 'async';
        var marker;
        slink.onload = () => {
            var map = L.map('amap');
            L.tileLayer('//{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
		attribution: '© <a href="http://osm.org/copyright">OpenStreetMap</a> contributors',
		maxZoom: 19
            }).addTo(map);
            if (position) {
                position = JSON.parse(position);
                map.setView(position, 16);
            } else {
                map.fitWorld();
                position = [0, 0];
            }
            var me = d.getElementById('amap');
            marker = L.marker(position, {
                'draggable': true,
                'autoPan': true,
                'autoPanPadding': [me.clientWidth/4, me.clientHeight/4],
            });
            marker.addTo(map);
            me.focus();
        }
        h.append(slink);

        let b = d.createElement("button");
        b.innerHTML = "Ok";
        f.addEventListener('submit', presubmit);
        f.appendChild(b);

        function presubmit() {
            let pos = marker.getLatLng();
            lat.value = pos.lat;
            lng.value = pos.lng;
            localStorage.setItem('lastpos', `[${pos.lat},${pos.lng}]`)
        }
        function keyHandler(e) {
            console.log("In keyhandler", e);
            switch(e.code) {
            case 'Escape':
                e.target.closest('form').remove();
                event.target.disabled = false;
                event.target.focus();
                break;
            case 'Enter':
                presubmit();
                f.submit();
                break;
            default:
                return true;
            };
            e.preventDefault();
            e.stopPropagation();
            return false;
        }

        let c = d.createElement("button");
        c.innerHTML = "&#x1f5d9;";
        c.className = 'close';
        c.title = 'close';
        c.onclick = e => {
            e.target.closest('form').remove();
            event.target.disabled = false; // The old event creating this form
            event.target.focus();
        };
        f.appendChild(c);
        f.addEventListener('keydown', keyHandler);
        p.append(f);
    }

    p = d.createElement("div");
    p.className = 'admbuttons';
    r = d.createElement("button");
    r.onclick = rotate;
    r.innerHTML = "\u27f2";
    r.dataset.angle = "-90";
    r.title = "Rotate left";
    p.appendChild(r);
    p.appendChild(d.createTextNode(" "));
    r = d.createElement("button");
    r.onclick = rotate;
    r.innerHTML = "\u27f3";
    r.dataset.angle = "90";
    r.title = "Rotate right";
    p.appendChild(r);

    p.appendChild(d.createTextNode(" "));
    r = d.createElement("button");
    r.onclick = e => tag_form(e, 'tag');
    r.innerHTML = "&#x1f3f7;";
    r.title = "Tag";
    r.accessKey = "t";
    p.appendChild(r);

    p.appendChild(d.createTextNode(" "));
    r = d.createElement("button");
    r.onclick = e => tag_form(e, 'person');
    r.innerHTML = "\u263a";
    r.title = "Person";
    r.accessKey = "p";
    p.appendChild(r);

    p.appendChild(d.createTextNode(" "));
    r = d.createElement("button");
    r.onclick = e => location_form(e);
    r.innerHTML = "\u{1f5fa}";
    r.title = "Location";
    r.accessKey = "l";
    p.appendChild(r);

    p.appendChild(d.createTextNode(" "));
    r = d.createElement("button");
    r.onclick = e => grade_form(e);
    r.innerHTML = "\u2606";
    r.title = "Grade";
    r.accessKey = "g";
    p.appendChild(r);
    details.appendChild(p);
})(document)
