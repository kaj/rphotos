<!doctype html>
<html>
  <head>
    <title>Photo details</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    <link rel="stylesheet" href="/static/photos.css" type="text/css"/>
  </head>
  <body>
    <h1>Photo details</h1>
    <p>{{photo.path}}</p>
    <p><img src="/view/{{photo.id}}"></p>
    {{#photo.grade}}<p>Betyg: {{.}}</p>{{/photo.grade}}
    <p>People: {{#people}}<a href="/person/{{slug}}">{{name}}</a>, {{/people}}</p>
    <p>Places: {{#places}}<a href="/place/{{slug}}">{{place}}</a>, {{/places}}</p>
    <p>Tags: {{#tags}}<a href="/tag/{{slug}}">{{tag}}</a>, {{/tags}}</p>
  </body>
</html>
