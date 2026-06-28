// js functions we want to call from rust

function async_sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

function set_cookie(value) {
    // Pin to the site root so a single canonical cookie is shared by every path the UI is
    // served under, instead of the writing document's default path.
    document.cookie = value + "; path=/";
}

// name is PBSAuthCookie, PVEAuthCookie or PMGAuthCookie
function clear_auth_cookie(name) {
    // Sweep the root path and any path the cookie may previously have been pinned to, so a stale
    // ticket cannot survive the clear.
    var gone = "=; expires=Thu, 01-Jan-1970 00:00:01 GMT";
    document.cookie = name + gone + "; path=/";
    document.cookie = name + gone;
    document.cookie = name + gone + "; path=" + location.pathname;
}

function get_cookie() {
    return document.cookie;
}

export {
    async_sleep,
    get_cookie,
    set_cookie,
    clear_auth_cookie,
};
