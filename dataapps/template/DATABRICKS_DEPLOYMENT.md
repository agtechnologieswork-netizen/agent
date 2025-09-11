# Databricks Apps Deployment Guide

This template is configured for deployment to Databricks Apps, supporting both Python backend and Node.js frontend.

## Prerequisites

- Databricks workspace with Apps enabled
- Databricks CLI installed and configured
- Access to the workspace where you want to deploy

## Template Structure

The template includes the following files for Databricks Apps deployment:

- `app.yaml` - Databricks App configuration
- `package.json` - Node.js dependencies and scripts (includes start command)
- `pyproject.toml` - Python dependencies and project configuration
- `Dockerfile` - Container configuration
- `.gitignore` - Files to exclude during deployment

## Deployment Process

### 1. Prepare the Application

The template is already configured with the necessary files. The deployment will follow Databricks Apps logic:

1. **Detect Node.js**: Since `package.json` is present at root, Databricks detects this as a hybrid app
2. **Install Node.js dependencies**: `npm install` (runs `cd frontend/reactadmin && npm install`)
3. **Install Python dependencies**: `uv sync` (from root)
4. **Build frontend**: `npm run build` (installs frontend deps and builds from `frontend/reactadmin/`)
5. **Start application**: `npm run start` (installs frontend deps, builds frontend, starts backend)

### 2. Deploy via Databricks CLI

1. **Sync files to workspace:**
   ```bash
   databricks sync --watch . /Workspace/Users/your-email@org.com/data-apps-template
   ```

2. **Deploy the app:**
   ```bash
   databricks apps deploy data-apps-template \
      --source-code-path /Workspace/Users/your-email@org.com/data-apps-template
   ```

### 3. Deploy via Databricks UI

1. Upload the app files to your Databricks workspace
2. Go to **Compute** → **Apps**
3. Click **Deploy** and select the folder with your app files
4. Review configuration and click **Deploy**

## Application Architecture

### Backend (Python/FastAPI)
- Located in `backend/` directory
- Provides REST API compatible with React Admin
- Uses Polars for data processing
- Serves customer data from CSV file

### Frontend (React/TypeScript)
- Located in `frontend/reactadmin/` directory
- React Admin interface for data management
- Built with Vite and Material-UI
- Serves static files from `/` route

### Main Entry Point
- `npm run start` handles everything:
  - Installs frontend dependencies (`cd frontend/reactadmin && npm install`)
  - Builds frontend (`npm run build`)
  - Starts backend (`uv run python backend/main.py`)
- Backend serves API at `/api` and frontend static files at `/`
- Handles CORS for cross-origin requests
- No duplicate dependencies - all frontend deps are in `frontend/reactadmin/package.json`

## Configuration

### Environment Variables
- `PORT`: Application port (default: 8000)
- `CORS_ORIGINS`: CORS allowed origins (default: "*")

### Resource Allocation
- Memory: 2Gi
- CPU: 1 core

## Customization

### Adding New Data Sources
1. Create new route in `backend/routes/`
2. Add corresponding schema in `backend/routes/*/schema.py`
3. Update the main app to include the new router

### Modifying Frontend
1. Edit components in `frontend/reactadmin/src/`
2. The build process will automatically compile changes
3. Static files are served from the root path

### Environment Configuration
Modify `app.yaml` to:
- Change resource allocation
- Add environment variables
- Modify the startup command

## Troubleshooting

### Common Issues

1. **Frontend not loading**: Ensure `npm run build` completed successfully
2. **API not accessible**: Check CORS configuration and port settings
3. **Build failures**: Verify all dependencies are in `pyproject.toml` and `package.json`

### Logs
View application logs in the Databricks Apps interface under the **Logs** tab.

### Local Testing
Test locally before deployment:
```bash
# Install all dependencies (both Node.js and Python)
npm run install:all

# Or run everything in one command
npm run start
```

**Note**: If you encounter Python environment issues locally, you may need to use a virtual environment:
```bash
# Create and activate virtual environment
python3 -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Then run the installation
npm run install:all
```

## File Structure

```
dataapps/template/
├── app.yaml                 # Databricks App configuration
├── package.json             # Node.js dependencies and start script
├── pyproject.toml           # Python dependencies and project configuration
├── Dockerfile              # Container configuration
├── .gitignore              # Deployment exclusions
├── backend/                # Python FastAPI backend
│   ├── main.py
│   ├── react_admin_helpers.py
│   └── routes/
└── frontend/reactadmin/    # React frontend
    ├── src/
    ├── package.json
    └── vite.config.ts
```

## Next Steps

After successful deployment:
1. Access your app through the Databricks Apps interface
2. Customize the data sources and UI as needed
3. Add authentication if required
4. Scale resources based on usage
