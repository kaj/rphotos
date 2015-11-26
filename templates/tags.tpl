<!doctype html>
<html>
  <head>
    <title>Photo tags</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    <link rel="stylesheet" href="/static/photos.css" type="text/css"/>
  </head>
  <body>
    <h1>Photo tags</h1>

    <ul class="alltags">{{#tags}}
      <li><a href="/tag/{{slug}}">{{tag}}</a>
    {{/tags}}</ul>
    </div>
  </body>
</html>
