<!doctype html>
<html>
  <head>
    <title>login</title>
    <meta http-equiv="Content-Type" content="text/html;charset=utf-8" />
    {{{csslink}}}
  </head>
  <body>
    {{> head}}
    <h1>login</h1>

    <form action="/login" method="post">
      <p><label for="user">User:</label>
	<input id="user" name="user"></p>
      <p><label for="password">Password:</label>
	<input id="password" name="password" type="password"></p>
      <p><input type="submit" value="Log in"></p>
    </form>
  </body>
</html>
