const express = require('express');
const cors = require('cors');

const app = express();
const port = process.env.PORT || 3001;

app.use(cors());
app.use(express.json());

app.get('/api/hello', (req, res) => {
  res.json({ message: 'Hello from simple template!', timestamp: new Date().toISOString() });
});

app.listen(port, () => {
  console.log(`Simple template server running on port ${port}`);
});
