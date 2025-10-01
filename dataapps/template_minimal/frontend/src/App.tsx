import { Admin, CustomRoutes } from 'react-admin';
import simpleRestProvider from 'ra-data-simple-rest';
import './App.css';
import { Route } from 'react-router-dom';

const dataProvider = simpleRestProvider('/api');

const PlaceholderDashboard = () => (
  <div>
    <div className="gradient"></div>
    <div className="grid"></div>
    <div className="container">
      <h1 className="title">Under Construction</h1>
      <p className="description">
        Your app is under construction. It's being built right now!
      </p>
      <div className="dots">
        <div className="dot"></div>
        <div className="dot"></div>
        <div className="dot"></div>
      </div>
      <footer className="footer">
        Built with ❤️ by{' '}
        <a href="https://app.build" target="_blank" className="footer-link">
          app.build
        </a>
      </footer>
    </div>
  </div>
);

export const App = () => (
  <Admin>
    {/* TODO: Add your resources here */}
    {/* 
    Example usage:
    
    import { Resource, ListGuesser, EditGuesser, ShowGuesser } from "react-admin";
    
    <Resource 
      name="users" 
      list={ListGuesser}
      edit={EditGuesser}
      show={ShowGuesser}
    />
    */}
    <CustomRoutes>
      <Route path="/" element={<PlaceholderDashboard />} />
    </CustomRoutes>
  </Admin>
);
