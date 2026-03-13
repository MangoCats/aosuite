var digitalData={};
var createObject=function(a,d){const b=a.split(".");
var c=digitalData;
for(var e=0;
e<b.length;
e++){var a=b[e];
if(!c[a]&&e!=b.length-1){c[a]={}
}else{if(e==b.length-1){c[a]=d
}}c=c[a]
}};
function analyticsClickTrigger(c,a,b){$(c).click(function(){createObject(a,b)
})
}function analyticsSubmit(c,a,b){$(c).submit(function(d){createObject(a,b)
})
}function clickWithStopPropagation(c,a,b){$(c).click(function(d){d.stopPropagation();
d.stopImmediatePropagation();
createObject(a,b)
})
}$(document).ready(function(){$('a[href*="/contactUs"]').click(function(){createObject("contactUs.formState","origination")
})
});
function siteSection(b){var c=typeof b.breadcrumbs[1]=="undefined"?"":b.breadcrumbs[1].linkText;
var a=typeof b.breadcrumbs[2]=="undefined"?"":b.breadcrumbs[2].linkText;
var d=typeof b.breadcrumbs[3]=="undefined"?"":b.breadcrumbs[3].linkText;
createObject("page.category.siteSection",c);
createObject("page.category.siteSection2",a);
createObject("page.category.siteSection3",d)
}function SearchTerm(a){$(a).keyup(function(b){var c=$(this).val();
if(c.length>2){createObject("collection.searchTerm",c)
}})
}function marketingClickCheckbox(c,b,a){$(c).click(function(){var d=$("#marketingconsent-yes");
if(d.is(":checked")){createObject(b,a)
}})
}var fieldMap=new Map([["email_address","user-email"],["first_name","first-name"],["last_name","last-name"],["reg_password","create-password"],["country","territory"],["organization","organization"],["jobTitle","job-title"]]);
function registerEvent(a){createObject("user.event",fieldMap.get(a));
$("#register :input").focus(function(c){var b=$(this).attr("id");
createObject("user.event",fieldMap.get(b))
})
}function cancelRegistration(c,a,b){$(c).click(function(d){d.preventDefault();
createObject(a,b);
window.location.href=$(d.target).attr("href")
})
}function searchResultLoaded(a){if($(a).children().length>0){createObject("internalSearch.event","search-success")
}}function searchSubmit(b,a){$(b).submit(function(c){createObject(a,$("#searchField").val())
})
}function searchSubmitFromResults(b,a){$(b).click(function(){createObject(a,$("#searchfield-srp").val())
})
}function digitalDataUserStatus(a){if(a==true){createObject("user.loggedInStatus","logged-in")
}else{createObject("user.loggedInStatus","anonymous")
}}function loginUserId(b,a){var c=getCookieByMatch(a)[0];
c=c.replace("pwc-id=","");
createObject(b,c)
}function registrationStart(b,a){$("#login :input").one("focus",function(){createObject(b,a)
})
};