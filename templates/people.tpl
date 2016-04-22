<!doctype html>
<html>
  <head>
    <title>Photo people</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    {{{csslink}}}
  </head>
  <body>
    {{> head}}
    <h1>Photo people</h1>

    <ul class="allpeople">{{#people}}
      <li><a href="/person/{{slug}}">{{name}}</a>
    {{/people}}</ul>
    </div>
  </body>
</html>
