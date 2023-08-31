// js functions we want to call from rust

function async_sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

function set_cookie(value) {
    document.cookie = value;
}

// name is PBSAuthCookie, PVEAuthCookie or PMGAuthCookie
function clear_auth_cookie(name) {
    document.cookie = name + "=; expires=Thu, 01-Jan-1970 00:00:01 GMT;";
}

function get_cookie() {
    return document.cookie;
}

function uplot(opts, data, node) {
    return new uPlot(opts, data, node);
}

function uplot_set_data(uplot, data) {
    uplot.setData(data);
}

function uplot_set_size(uplot, width, height) {
    uplot.setSize({
	width: width,
	height: height,
    });
}

export {
    async_sleep,
    get_cookie,
    set_cookie,
    clear_auth_cookie,
    uplot,
    uplot_set_data,
    uplot_set_size,
};
