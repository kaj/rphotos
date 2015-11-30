<!doctype html>
<html>
  <head>
    <title>Photo index</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    <link rel="stylesheet" href="/static/photos.css" type="text/css"/>
  </head>
  <body>
    <h1>Photo index</h1>

    <p><a href="/tag/">Taggar</a>
      · <a href="/person/">Personer</a>
      · <a href="/place/">Platser</a></p>

    <div class="photos">
    {{#photos}}
    <p><a href="/details/{{id}}"><img src="/icon/{{id}}"></a></p>
    {{/photos}}
    </div>
  </body>
</html>
