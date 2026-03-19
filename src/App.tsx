import { Routes, Route } from "react-router-dom";
import ProjectList from "@/pages/ProjectList";
import ProjectDetail from "@/pages/ProjectDetail";
import DuplicateReview from "@/pages/DuplicateReview";

function App() {
  return (
    <div className="min-h-screen bg-background">
      <Routes>
        <Route path="/" element={<ProjectList />} />
        <Route path="/project/:id" element={<ProjectDetail />} />
        <Route path="/project/:id/dedup" element={<DuplicateReview />} />
      </Routes>
    </div>
  );
}

export default App;
