<!doctype html>
<html>
  <head>
    <title>Photos tagged {{tag.tag}}</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    <link rel="stylesheet" href="/static/photos.css" type="text/css"/>
  </head>
  <body>
    {{> head}}
    <h1>{{tag.tag}}</h1>

    <div class="photos">
    {{#photos}}
    <p><a href="/details/{{id}}"><img src="/img/{{id}}/s"></a></p>
    {{/photos}}
    </div>
  </body>
</html>
