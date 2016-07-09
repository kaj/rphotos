<!doctype html>
<html>
  <head>
    <title>Photo details</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    {{{csslink}}}
  </head>
  <body>
    {{> head}}
    <h1>Photo details</h1>

    <p><a href="/img/{{photo.id}}/l">{{photo.path}}</a></p>
    <p><img src="/img/{{photo.id}}/m"></p>

    {{#position}}
    <div id="map" style="height: 8em;width: 40%;float: right;border: solid 1px #666;"> </div>
    <link href="https://rasmus.krats.se/static/leaflet077c/leaflet.css" rel="stylesheet"/>
    <script language="javascript" src="https://rasmus.krats.se/static/leaflet077c/leaflet.js" type="text/javascript">
    </script>
    <script language="javascript" type="text/javascript">
    var pos = [{{x}}, {{y}}];
    var map = document.getElementById('map');
    map.style.height = 3 * map.clientWidth / 4 + "px";
    var map = L.map('map').setView(pos, 16);
    L.tileLayer('//{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
    attribution: '© <a href="http://osm.org/copyright">OpenStreetMap</a> contributors'
    }).addTo(map);
    L.marker(pos).addTo(map);
    </script>
    {{/position}}

    {{#photo.grade}}<p>Betyg: {{.}}</p>{{/photo.grade}}
    {{#photo}}<p>Tid: {{date}}</p>{{/photo}}
    <p>People: {{#people}}<a href="/person/{{slug}}">{{person_name}}</a>, {{/people}}</p>
    <p>Places: {{#places}}<a href="/place/{{slug}}">{{place_name}}</a>, {{/places}}</p>
    <p>Tags: {{#tags}}<a href="/tag/{{slug}}">{{tag_name}}</a>, {{/tags}}</p>
    {{#position}}<p>Position: {{x}} {{y}}</p>{{/position}}
  </body>
</html>
