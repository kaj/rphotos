<header>
<span><a href="/">Bilder</a>
{{#lpath}} - <a href="{{url}}">{{name}}</a>{{/lpath}}
</span>
<span>· <a href="/tag/">Taggar</a></span>
<span>· <a href="/person/">Personer</a></span>
<span>· <a href="/place/">Platser</a></span>
{{#user}}<span class="user">{{.}}
(<a href="/logout">log out</a>)
</span>{{/user}}
{{^user}}<span class="user">(<a href="/login">log in</a>)</span>{{/user}}
</header>
