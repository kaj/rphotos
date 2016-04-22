<!doctype html>
<html>
  <head>
    <title>Photos with {{place.place}}</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    {{{csslink}}}
  </head>
  <body>
    {{> head}}
    <h1>{{place.place}}</h1>

    <div class="group">
    {{#photos}}
    <p class="item"><a href="/details/{{id}}"><img src="/img/{{id}}/s"></a></p>
    {{/photos}}
    </div>
  </body>
</html>
