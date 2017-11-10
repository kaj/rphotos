// Admin functionality for rphotos
function rpadmin() {
    var details = document.querySelector('.details');

    function rotate(event) {
        var imgid = details.dataset.imgid;
        var angle = event.target.dataset.angle;
        var r = new XMLHttpRequest();
        document.body.classList.add('busy');
        r.open('POST', '/adm/rotate');
        r.onload = function() {
            if (r.status === 200) {
                // Yay!
                document.location.reload(true);
            } else {
                alert("Rotating failed: " + r.status);
            }
            document.body.classList.remove('busy');
        }
        r.onerror = function() {
            alert("Rotating failed.");
            document.body.classList.remove('busy');
        }
        r.ontimeout = function() {
            alert("Rotating timed out");
            document.body.classList.remove('busy');
        }
        r.setRequestHeader("Content-type", "application/x-www-form-urlencoded");
        r.send("angle=" + angle + "&image=" + imgid)
    }

    var list;

    function tag_form(event, category) {
        event.target.disabled = true;
        var imgid = details.dataset.imgid;
        var f = document.createElement("form");
        f.className = category;
        f.action = "/adm/" + category;
        f.method = "post";
        var i = document.createElement("input");
        i.type="hidden";
        i.name="image";
        i.value = imgid;
        f.appendChild(i);
        i = document.createElement("input");
        i.type = "text";
        i.autocomplete="off";
        i.tabindex="1";
        i.name = category;
        i.addEventListener('keyup', e => {
            let c = e.code;
            if (c == 'ArrowUp' || c == 'ArrowDown' || c == 'Escape' || c == 'Enter') {
		return true;
            }
            let i = e.target, v = i.value;
            if (v.length > 0) {
		let r = new XMLHttpRequest();
		r.onload = function() {
                    let t = JSON.parse(this.responseText);
                    list.innerHTML = '';
                    t.map(x => {
			let a = document.createElement('a');
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
		r.open('GET', document.location.origin + '/ac/' + category + '?q=' + encodeURIComponent(v));
		r.send(null);
            } else {
		list.innerHTML = '';
            }
            e.preventDefault();
            e.stopPropagation();
            return false;
	});
        f.appendChild(i);
        list = document.createElement("div");
        list.className = "completions";
        f.appendChild(list);
        f.addEventListener('keypress', e => {
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
        meta.appendChild(f);
        i.focus();
    }

    var meta = details.querySelector('.meta');
    if (meta) {
        p = document.createElement("p");
        r = document.createElement("button");
        r.onclick = rotate;
        r.innerHTML = "\u27f2";
        r.dataset.angle = "-90";
        r.title = "Rotate left";
        p.appendChild(r);
        p.appendChild(document.createTextNode(" "));
        r = document.createElement("button");
        r.onclick = rotate;
        r.innerHTML = "\u27f3";
        r.dataset.angle = "90";
        r.title = "Rotate right";
        p.appendChild(r);

        p.appendChild(document.createTextNode(" "));
        r = document.createElement("button");
        r.onclick = e => tag_form(e, 'tag');
        r.innerHTML = "&#x1f3f7;";
        r.title = "Tag photo";
        r.accessKey = "T";
        p.appendChild(r);

        p.appendChild(document.createTextNode(" "));
        r = document.createElement("button");
        r.onclick = e => tag_form(e, 'person');
        r.innerHTML = "\u263a";
        r.title = "Person in picture";
        r.accessKey = "P";
        p.appendChild(r);
        meta.appendChild(p);
    }
}
