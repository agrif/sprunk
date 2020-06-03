// set this to wherever icecast is hosted
var ICECAST_BASE = '';

function status(f) {
    return fetch(ICECAST_BASE + 'status-json.xsl')
        .then(response => response.json())
        .then(function(d) {
            var stations = d['icestats']['source'];
            for (var i = 0; i < stations.length; i++) {
                var idparts = stations[i]['listenurl'].split('/');
                var id = idparts[idparts.length - 1];
                var nameparts = (stations[i]['title'] || '').split(' - ');
                var name = nameparts[0];
                if (nameparts.length <= 1) {
                    name = id;
                }
                
                stations[i]['name'] = name;
                stations[i]['id'] = id;
            }
            return stations;
        }).then(f);
}

var current = null;
function update() {
    if (current == null)
        return;
    status(stations => {
        for (var i = 0; i < stations.length; i++) {
            if (current != stations[i]['id'])
                continue;
            var np = document.getElementById('nowplaying');
            np.innerHTML = stations[i]['title'].replace(/ - /g, '<br />');
            document.title = stations[i]['title'];
        }
    });
}

function play(name) {
    var audio = document.getElementById('player');
    var npi = document.getElementById('nowplayingicon');
    npi.src = 'icons/' + name + '.png';
    audio.src = ICECAST_BASE + name;
    audio.play();
    current = name;
    window.location.hash = name;
    update();
}

function draw(canvas, g, data, analyser) {
    var bins = 20;
    var startF = 20;
    var endF = 15000; // sorry if you can hear above this
    var startP = startF / (analyser.context.sampleRate / 2.0);
    var endP = endF / (analyser.context.sampleRate / 2.0);
    var startI = Math.round(Math.max(1, data.length * startP));
    var endI = Math.round(Math.min(data.length - 1, data.length * endP));

    requestAnimationFrame(() => draw(canvas, g, data, analyser));

    g.clearRect(0, 0, canvas.width, canvas.height);
    analyser.getByteFrequencyData(data);

    function freq(i) {
        return Math.log2(i);
    }

    var start = freq(startI);
    var end = freq(endI - 1);
    // x = (freq(i) - start) / (end - start)
    var m = 1.0 / (end - start);
    var b = -start * m;

    var bin = 0;
    var binaccum = 0.0;
    var bincount = 0;

    g.lineWidth = 0;
    g.strokeStyle = 'rgb(255, 255, 255)';
    g.fillStyle = 'rgb(255, 255, 255)';
    for (var i = startI; i < endI + 1; i++) {
        var curbin = Math.floor((m * freq(i) + b) * bins);
        if (curbin > bin) {
            // draw
            g.beginPath();
            var h = canvas.height * binaccum / bincount;
            var x = canvas.width * bin / bins;
            g.rect(x, canvas.height - h,
                   canvas.width / bins - 2, h);
            g.fill();

            bin = curbin;
            binaccum = 0.0;
            bincount = 0;
        }

        // max
        if (data[i] > binaccum)
            binaccum = data[i];
        bincount = 256.0;

        // average
        //binaccum += data[i] / 256.0;
        //bincount += 1;
    }

    // debug
    if (false) {
        g.lineWidth = 2;
        g.strokeStyle = 'rgb(255, 0, 0)';
        g.beginPath();
        for (var i = startI; i < endI; i++) {
            var h = canvas.height * data[i] / 256.0;
            var x = canvas.width * (m * freq(i) + b);
            if (i == 0)
                g.moveTo(x, canvas.height - h);
            else
                g.lineTo(x, canvas.height - h);
        }
        g.stroke();
    }
}

status(stations => {
    var icons = document.getElementById('icons');
    icons.innerHTML = '';

    for (var i = 0; i < stations.length; i++) {
        var icon = document.createElement('img');
        icon.src = 'icons/' + stations[i]['id'] + '.png';
        icon.alt = stations[i]['name'];
        icon.title = stations[i]['name'];
        var a = document.createElement('a');
        a.appendChild(icon);
        (function (capturename) {
            a.onclick = () => play(capturename);
        })(stations[i]['id']);
        icons.appendChild(a);
    }

    if (window.location.hash) {
        play(window.location.hash.substring(1));
    }
    window.setInterval(update, 5000);

    // web audio funny business for analyser
    var ctx = new (window.AudioContext || window.webkitAudioContext)();
    var analyser = ctx.createAnalyser();
    var audio = document.getElementById('player');
    var source = ctx.createMediaElementSource(audio);
    source.connect(analyser);
    analyser.connect(ctx.destination);
    analyser.fftSize = 4096;
    //analyser.minDecibels = -150;
    //analyser.maxDecibels = -30;

    var data = new Uint8Array(analyser.frequencyBinCount);
    var canvas = document.getElementById('visual');
    var g = canvas.getContext('2d');
    draw(canvas, g, data, analyser);    
});

