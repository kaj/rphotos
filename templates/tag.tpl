<!doctype html>
<html>
  <head>
    <title>Photos tagged {{tag.tag}}</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    {{{csslink}}}
  </head>
  <body>
    {{> head}}
    <h1>{{tag.tag}}</h1>

    <div class="group">
    {{#photos}}
    <p class="item"><a href="/details/{{id}}"><img src="/img/{{id}}/s"></a></p>
    {{/photos}}
    </div>
  </body>
</html>
