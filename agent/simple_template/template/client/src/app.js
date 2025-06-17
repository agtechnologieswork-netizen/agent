async function fetchData() {
    try {
        const response = await fetch('http://localhost:3001/api/hello');
        const data = await response.json();
        document.getElementById('response').innerHTML = 
            `<h3>API Response:</h3><pre>${JSON.stringify(data, null, 2)}</pre>`;
    } catch (error) {
        document.getElementById('response').innerHTML = 
            `<h3>Error:</h3><p>${error.message}</p>`;
    }
}

window.addEventListener('load', () => {
    console.log('Simple template client loaded');
});
