<!doctype html>
<html>
  <head>
    <title>Photo places</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    <link rel="stylesheet" href="/static/photos.css" type="text/css"/>
  </head>
  <body>
    {{> head}}
    <h1>Photo places</h1>

    <ul class="allplaces">{{#places}}
      <li><a href="/place/{{slug}}">{{place}}</a>
    {{/places}}</ul>
    </div>
  </body>
</html>
