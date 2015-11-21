<!doctype html>
<html>
  <head>
    <title>Photo index</title>
  </head>
  <body>
    <h1>Photo index</h1>
    <p>How nice is this?</p>
    {{#photos}}
    <p><a href="/icon/{{id}}">{{path}}</a></p>
    {{/photos}}
  </body>
</html>
