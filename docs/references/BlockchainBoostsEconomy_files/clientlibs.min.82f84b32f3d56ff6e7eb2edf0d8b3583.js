$(document).ready(function(){$(".social-share-rebrand").on("click",function(){event.preventDefault();
$(this).toggleClass("active");
$(this).find(".social-share-rebrand-overlay").toggleClass("active");
a();
$(".social-share-rebrand-copy").show();
$(".social-share-rebrand-copied").hide();
b(top);
$(function(){var c=$(".social-share-rebrand").offset().top;
b(c-$(window).scrollTop());
b(c-$(window).scrollTop())
})
});
$(".social-share-rebrand-copy").on("click",function(){event.preventDefault();
event.stopPropagation();
var c=$("<input>");
$("body").append(c);
c.val($("#social-share-input").val()).select();
document.execCommand("copy");
c.remove();
$(this).hide();
$(".social-share-rebrand-copied").show();
$(".social-share-rebrand-copied").addClass("copied");
$(this).siblings("input").select()
});
$(".social-share-rebrand-overlay").on("click",function(){event.preventDefault();
event.stopImmediatePropagation();
event.stopPropagation()
});
$(".social-share-rebrand-close").on("click",function(){event.preventDefault();
event.stopPropagation();
a();
$(".social-share-rebrand-overlay").toggleClass("active");
$(".social-share-rebrand").toggleClass("active");
$(".social-share-rebrand-copied").hide();
$(".social-share-rebrand-copy").show()
});
$(window).scroll(function(){b(top);
$(function(){var c=$(".social-share-rebrand").offset().top;
b(c-$(window).scrollTop());
b(c-$(window).scrollTop())
})
});
function b(i){var g=$(".social-share-rebrand").offset().left;
var c=$(window).height();
var h=c-i;
var j=i+50+"px";
var e=i-180+"px";
var f=g-20+"px";
var d=g-245+"px";
if(h<200){$(".social-share-rebrand-overlay").css("top",e);
$(".social-share-rebrand-overlay").css("left",f);
$(".social-share-rebrand-overlay").removeClass("icon-bottom");
if($("body").hasClass("page-dir--rtl")){$(".social-share-rebrand-overlay").css("left",d)
}}else{$(".social-share-rebrand-overlay").css("top",j);
$(".social-share-rebrand-overlay").css("left",f);
$(".social-share-rebrand-overlay").addClass("icon-bottom");
if($("body").hasClass("page-dir--rtl")){$(".social-share-rebrand-overlay").css("left",d)
}}}$(".social-share-rebrand").keyup(function(c){if(c.keyCode===13){$(this).click()
}});
$(".social-share-rebrand-copy").keyup(function(c){if(c.keyCode===13){c.preventDefault();
c.stopPropagation();
$(this).click();
$(".social-share-rebrand-overlay").addClass("active");
if($(".social-share-rebrand-copy").css("display")=="inline-block"){$(".social-share-rebrand-copy").hide()
}if($(".social-share-rebrand-copied").css("display")=="none"){$(".social-share-rebrand-copied").show()
}}});
$(".social-share-rebrand-close").keyup(function(c){if(c.keyCode===13){$(this).click();
$(".social-share-rebrand-overlay").removeClass("active");
$(".social-share-rebrand").removeClass("active")
}});
function a(){var c=$(".social-share-rebrand-overlay").attr("aria-expanded");
if(c==="true"){c="false"
}else{c="true"
}$(".social-share-rebrand-overlay").attr("aria-expanded",c)
}$(document).click(function(c){if(!$(c.target).parents().andSelf().is(".social-share-rebrand")){if($(".social-share-rebrand-overlay").hasClass("active")){$(".social-share-rebrand-overlay").removeClass("active");
if($(".social-share-rebrand").hasClass("active")){$(".social-share-rebrand").removeClass("active")
}a()
}}});
$(window).keyup(function(c){if(c.keyCode===27){$(".social-share-rebrand-overlay").removeClass("active");
$(".social-share-rebrand-copied").hide();
$(".social-share-rebrand-copy").show()
}});
$("#social-share-input").dblclick(function(){$(this).select()
})
});