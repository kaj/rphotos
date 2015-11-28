<!doctype html>
<html>
  <head>
    <title>Photo people</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    <link rel="stylesheet" href="/static/photos.css" type="text/css"/>
  </head>
  <body>
    <h1>Photo people</h1>

    <ul class="allpeople">{{#people}}
      <li><a href="/person/{{slug}}">{{name}}</a>
    {{/people}}</ul>
    </div>
  </body>
</html>
