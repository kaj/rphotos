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

    var meta = details.querySelector('.meta');
    if (meta) {
	p = document.createElement("p");
	r = document.createElement("button");
	r.onclick = rotate;
	r.innerHTML = "\u27f2";
	r.dataset.angle = "-90";
	r.title = "Rotate left";
	p.appendChild(r);
	r = document.createElement("button");
	r.onclick = rotate;
	r.innerHTML = "\u27f3";
	r.dataset.angle = "90";
	r.title = "Rotate right";
	p.appendChild(r);
	meta.appendChild(p);
    }
}
