$(document).ready(function(){$("a[pwcbtnrendition]").addClass("btn-cta");
$("a[pwcbtnfile]").each(function(){if($("a[pwcbtnfile]").hasClass("btn")){$(this).addClass("btn-cta-common")
}});
$('a[pwcbtnrendition="Red"]').addClass("btn-cta--red");
$('a[pwcbtnrendition="Orange"]').addClass("btn-cta--orange");
$('a[pwcbtnrendition="DigitalRose"]').addClass("btn-cta--digitalrose");
$('a[pwcbtnrendition="White"]').addClass("btn-cta--white");
$('a[pwcbtnrendition="Burgundy"]').addClass("btn-cta--burgundy");
$('a[pwcbtnicon="Chevron"]').addClass("btn-cta--chevron");
$('a[pwcbtnicon="Download"]').addClass("btn-cta--download");
$('a[pwcbtnicon="ExternalLink"]').addClass("btn-cta--external");
$("[pwcbtnrendition]").parent().addClass("btn-cta--parent");
$("[pwcbtnicon]").each(function(){$(this).append("<i></i>")
});
var a=0;
var b;
if($('[pwcbtnicon="Download"]').length>0){$('[pwcbtnicon="Download"]').each(function(c){if(document.getElementsByClassName("btn-cta--download")[c].hasAttribute("data-size")&&document.getElementsByClassName("btn-cta--download")[c].getAttribute("pwcbtnfile")=="ShowSize"){b=document.getElementsByClassName("btn-cta--download")[c].getAttribute("data-size");
$(this).wrap('<span class="download-btn--parent"></span>');
$(this).parent().append('<span class="downloadfileSize">'+b+"</span>");
c++
}})
}});