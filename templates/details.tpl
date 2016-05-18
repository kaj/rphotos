<!doctype html>
<html>
  <head>
    <title>Photo details</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    {{{csslink}}}
  </head>
  <body>
    {{> head}}
    <h1>Photo details</h1>

    <p><a href="/img/{{photo.id}}/l">{{photo.path}}</a></p>
    <p><img src="/img/{{photo.id}}/m"></p>
    {{#photo.grade}}<p>Betyg: {{.}}</p>{{/photo.grade}}
    {{#photo.date}}<p>Tid: {{year}}-{{month}}-{{day}}
      {{time}}</p>{{/photo.date}}
    <p>People: {{#people}}<a href="/person/{{slug}}">{{name}}</a>, {{/people}}</p>
    <p>Places: {{#places}}<a href="/place/{{slug}}">{{place}}</a>, {{/places}}</p>
    <p>Tags: {{#tags}}<a href="/tag/{{slug}}">{{tag}}</a>, {{/tags}}</p>
  </body>
</html>
