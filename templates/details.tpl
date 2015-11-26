<!doctype html>
<html>
  <head>
    <title>Photo details</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
  </head>
  <body>
    <h1>Photo details</h1>
    <p>{{photo.path}}</p>
    <p><img src="/view/{{photo.id}}"></p>
    <p>Tags: {{#tags}}<a href="/tag/{{slug}}">{{tag}}</a>, {{/tags}}</p>
  </body>
</html>
