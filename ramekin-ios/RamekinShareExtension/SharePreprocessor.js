var ExtensionPreprocessingJS = {
  run: function (args) {
    args.completionFunction({
      html: document.documentElement.outerHTML,
      url: location.href,
      title: document.title
    });
  }
};
