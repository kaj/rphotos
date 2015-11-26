<!doctype html>
<html>
  <head>
    <title>Photos tagged {{tag.tag}}</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
  </head>
  <body>
    <h1>{{tag.tag}}</h1>

    <div class="photos">
    {{#photos}}
    <p><a href="/details/{{id}}"><img src="/icon/{{id}}"></a></p>
    {{/photos}}
    </div>
  </body>
</html>
