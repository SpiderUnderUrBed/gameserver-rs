const ram_usage = document.getElementById('ram-usage');
const core_usage = document.getElementById('core-usage');

const basePath = document.querySelector('meta[name="site-url"]').content.replace(/\/$/, '');
let totalMemory = null;
let counter = 0;
let ram_chart = new Chart(ram_usage, {
    type: 'line',
    data: {
        labels: [],
        datasets: [{
            label: 'Used Memory (GB)',
            data: [],
            borderWidth: 2,
            borderColor: 'rgba(75, 192, 192, 1)',
            backgroundColor: 'rgba(75, 192, 192, 0.2)',
            fill: true,
            tension: 0.3
        }]
    },
    options: {
        maintainAspectRatio: false,
        animation: false,
        responsive: true,
        scales: {
            x: {
                title: { display: true, text: 'Time' }
            },
            y: {
                beginAtZero: true,
                title: { display: true, text: 'GB' },
            }
        }
    }
});
let core_chart = new Chart(core_usage, {
    type: 'line',
    data: {
        labels: [],
        datasets: []
    },
    options: {
        maintainAspectRatio: false,
        animation: false,
        responsive: true,
        scales: {
            x: {
                title: { display: true, text: 'Time' }
            },
            y: {
                beginAtZero: true,
                title: { display: true, text: 'Hz' },
            }
        }
    }
});


const statusEvent = new EventSource(`${basePath}/api/statistics`);
statusEvent.onmessage = (e) => {
    const payload = JSON.parse(e.data);
    const gbsUsed = Number(payload.used_memory) / (1024 * 1024);
    
    if (!totalMemory) {
        totalMemory = Number(payload.total_memory) / (1024 * 1024);
        ram_chart.options.scales.y.max = totalMemory;
        ram_chart.update('none');
    }
    ram_chart.data.labels.push(counter++);
    if (ram_chart.data.datasets[0].data.length > 20) {  
        ram_chart.data.labels.shift();
        ram_chart.data.datasets[0].data.shift();     
    }
    ram_chart.data.datasets[0].data.push(gbsUsed);
    
    let core_dataset = {
            label: 'Used Memory (GB)',
            data: [],
            borderWidth: 2,
            borderColor: 'rgba(75, 192, 192, 1)',
            backgroundColor: 'rgba(75, 192, 192, 0.2)',
            fill: true,
            tension: 0.3
        };
    
    // const maxPoints = 50;
    // if (chart.data.labels.length > maxPoints) {
    //     chart.data.labels.shift();
    //     chart.data.datasets[0].data.shift();
    // }
    
    ram_chart.update();
};