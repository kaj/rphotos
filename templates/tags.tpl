<!doctype html>
<html>
  <head>
    <title>Photo tags</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    {{{csslink}}}
  </head>
  <body>
    {{> head}}
    <h1>Photo tags</h1>

    <ul class="alltags">{{#tags}}
      <li><a href="/tag/{{slug}}">{{tag}}</a>
    {{/tags}}</ul>
    </div>
  </body>
</html>
