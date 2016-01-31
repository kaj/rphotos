<!doctype html>
<html>
  <head>
    <title>{{title}}</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    <link rel="stylesheet" href="/static/photos.css" type="text/css"/>
  </head>
  <body>
    {{> head}}
    <h1>{{title}}</h1>

    <div class="groups">
      {{#groups}}
      <div class="group"><h2>{{title}}</h2>
	<p><a href="{{url}}"><img src="/img/{{photo.id}}/s"></a></p>
	<p>{{count}} pictures</p>
      </div>
    {{/groups}}
    </div>
  </body>
</html>
