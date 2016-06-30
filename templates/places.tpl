<!doctype html>
<html>
  <head>
    <title>Photo places</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    {{{csslink}}}
  </head>
  <body>
    {{> head}}
    <h1>Photo places</h1>

    <ul class="allplaces">{{#places}}
      <li><a href="/place/{{slug}}">{{place_name}}</a>
    {{/places}}</ul>
    </div>
  </body>
</html>
