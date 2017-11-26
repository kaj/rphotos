(function(d) {
  let f = d.querySelector('footer');
  f.insertAdjacentHTML(
    'afterbegin',
    '<p><a href="#help" title="Help" accesskey="?">?</a></p>')
  d.querySelector('[href="#help"]').onclick = e => {
    if (d.getElementById('help') == null) {
      f.insertAdjacentHTML(
        'beforebegin',
        '<div id="help"><h2>Key bindings</h2>' +
        [].map.call(
          d.querySelectorAll('[accesskey]'),
          e => e.accessKeyLabel + ": " + (e.title || e.innerText)).join('<br/>') +
        '</div>');
    }
    return true;
  };
})(document)
