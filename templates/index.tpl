<!doctype html>
<html>
  <head>
    <title>Photo index</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    <link rel="stylesheet" href="/static/photos.css" type="text/css"/>
  </head>
  <body>
    {{> head}}
    <h1>Photo index</h1>

    <div class="photos">
    {{#photos}}
    <p><a href="/details/{{id}}"><img src="/icon/{{id}}"></a></p>
    {{/photos}}
    </div>
  </body>
</html>
